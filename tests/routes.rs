//! Route rung: the production HTTP routing table driven natively.
//!
//! `omnia_test::guest::Provider` implements every capability, and the
//! `router` bounds pick out which of its doubles a test seeds; `oneshot`
//! exercises the same `axum::Router` the WASI export serves, so a route,
//! codec, or handler regression surfaces here without a component build.
//!
//! `dispatch` checks **wiring, not semantics**: every `(method, path)` the
//! router registers must reach its handler. The remaining tests pin the
//! codec contracts that live in `src/lib.rs` itself (`handle_with` routes).
//! Business logic is covered by each crate's own tests.

use acme_common::routes;
use axum::body::{Body, to_bytes};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Method, Request, Response, StatusCode};
use omnia_test::guest::{MapConfig, Provider, ScriptedTables};
use tally_connector::TallyMessage;
use tower::ServiceExt as _;

const TALLY_MESSAGE: &[u8] = include_bytes!("../crates/tally-connector/data/tally-message.json");
const RECEIVE_MESSAGE: &[u8] = include_bytes!("../crates/pulse-connector/data/receive-message.xml");

/// Every `(method, path)` the router registers, in `src/lib.rs` order,
/// including each method a `.merge()` chain adds to one path.
const ROUTES: &[(Method, &str)] = &[
    (Method::POST, routes::http::APC),
    (Method::POST, routes::http::PULSE_XML),
    (Method::GET, routes::http::VEHICLE_INFO),
    (Method::POST, pattern::routes::DECODE),
    (Method::POST, pattern::routes::PLACES),
    (Method::GET, pattern::routes::NEARBY),
    (Method::POST, capability::routes::ARCHIVE),
    (Method::POST, capability::routes::ALERT),
    (Method::POST, capability::routes::NOTE),
    (Method::POST, capability::routes::READING),
    (Method::GET, docstore::paths::STOPS),
    (Method::POST, docstore::paths::STOPS),
    (Method::GET, docstore::paths::STOP),
    (Method::PUT, docstore::paths::STOP),
    (Method::DELETE, docstore::paths::STOP),
    (Method::GET, docstore::paths::ROUTES),
    (Method::POST, docstore::paths::ROUTES),
    (Method::GET, docstore::paths::ROUTE),
    (Method::GET, docstore::paths::STOP_TIMES),
    (Method::POST, docstore::paths::STOP_TIMES),
    (Method::GET, docstore::paths::STOP_TIME),
    (Method::GET, sql::paths::AGENCIES),
    (Method::POST, sql::paths::AGENCIES),
    (Method::GET, sql::paths::AGENCY),
    (Method::PATCH, sql::paths::AGENCY),
    (Method::GET, sql::paths::AGENCY_FEEDS),
    (Method::POST, sql::paths::AGENCY_FEEDS),
    (Method::GET, sql::paths::FEEDS),
    (Method::DELETE, sql::paths::FEED),
];

/// Default doubles with the one config key every handler reads.
///
/// `ScriptedTables` panics on an unscripted statement, so SQL is answered
/// with no rows and no affected rows: table-backed routes reach their
/// handler and answer for themselves instead of aborting the request.
fn provider() -> Provider {
    Provider::default()
        .config(MapConfig::default().with([("ENV", "dev")]))
        .tables(ScriptedTables::default().on_query(|_, _| true, Vec::new()).on_exec(|_, _| true, 0))
}

/// Replace each `{param}` segment of a route pattern with a concrete id.
fn concrete(path: &str) -> String {
    path.split('/')
        .map(|segment| if segment.starts_with('{') { "1" } else { segment })
        .collect::<Vec<_>>()
        .join("/")
}

async fn send(provider: Provider, request: Request<Body>) -> (StatusCode, Vec<u8>) {
    let response = guest::router(provider).oneshot(request).await.expect("response");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    (status, body.to_vec())
}

fn content_type(response: &Response<Body>) -> &str {
    response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok()).unwrap_or_default()
}

/// Whether the router, rather than a handler, produced this response.
///
/// axum answers an unregistered path with an empty `404` and a registered
/// path with the wrong method with `405`. A handler's own `not_found!`
/// carries its message in the body, so it counts as dispatched.
fn router_miss(status: StatusCode, body: &[u8]) -> bool {
    status == StatusCode::METHOD_NOT_ALLOWED || (status == StatusCode::NOT_FOUND && body.is_empty())
}

#[tokio::test]
async fn dispatch() {
    let provider = provider();
    let mut misses = Vec::new();
    for (method, pattern) in ROUTES {
        let path = concrete(pattern);
        let request = Request::builder()
            .method(method.clone())
            .uri(&path)
            .body(Body::empty())
            .expect("request");
        let (status, body) = send(provider.clone(), request).await;
        if router_miss(status, &body) {
            misses.push(format!("{method} {path} -> {status}"));
        }
    }
    assert!(misses.is_empty(), "routes not dispatched:\n{}", misses.join("\n"));
}

#[tokio::test]
async fn apc_tally() {
    let provider = provider();
    let request = Request::post(routes::http::APC)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(TALLY_MESSAGE))
        .expect("request");

    let (status, body) = send(provider.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_slice(), br#""OK""#);

    let published = provider.publish.sent();
    assert_eq!(published.len(), 1);
    let (topic, record) = &published[0];
    assert_eq!(topic, "dev-realtime-tally-apc.v2");

    let message: TallyMessage = serde_json::from_slice(TALLY_MESSAGE).expect("deserialize");
    let site = message.device.as_ref().expect("device").site.as_str();
    assert_eq!(record.headers.get("key").map(String::as_str), Some(site));
}

/// Malformed XML on the Pulse route: the codec contract is that it is
/// answered with the vendor's fault, not the framework's plain-text 400.
#[tokio::test]
async fn pulse_malformed_xml() {
    let request = Request::post(routes::http::PULSE_XML)
        .header(CONTENT_TYPE, "text/xml")
        .body(Body::from(&b"<garbage/>"[..]))
        .expect("request");

    let response = guest::router(provider()).oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(content_type(&response).starts_with("text/xml"), "{:?}", content_type(&response));
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    assert!(String::from_utf8_lossy(&body).contains("<Fault>"));
}

#[tokio::test]
async fn pulse_receive() {
    let request = Request::post(routes::http::PULSE_XML)
        .header(CONTENT_TYPE, "text/xml")
        .body(Body::from(RECEIVE_MESSAGE))
        .expect("request");

    let response = guest::router(provider()).oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    assert!(content_type(&response).starts_with("text/xml"), "{:?}", content_type(&response));
}

/// The `handle_with` GET whose body, not query string, is the request.
#[tokio::test]
async fn nearby_body() {
    let request = Request::get(pattern::routes::NEARBY)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(&br#"{"lat":-36.8442,"lon":174.7676,"radius_m":500}"#[..]))
        .expect("request");

    let (status, body) = send(provider(), request).await;

    assert!(status != StatusCode::BAD_REQUEST && !router_miss(status, &body), "{status}");
}

#[cfg(feature = "god-mode")]
#[tokio::test]
async fn set_trip() {
    let request = Request::post(concrete(routes::http::SET_TRIP))
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(&b"{}"[..]))
        .expect("request");

    let (status, body) = send(provider(), request).await;

    assert!(!router_miss(status, &body), "{status}");
}
