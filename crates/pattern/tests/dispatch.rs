//! Every route the crate registers reaches its handler through the router,
//! and the `handle_with` GET reads its request from the body.

use axum::body::{Body, to_bytes};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Method, Request, StatusCode};
use omnia_sdk::api::Client;
use omnia_test::guest::{Provider, ScriptedTables};
use pattern::routes;
use tower::ServiceExt as _;

/// Every `(method, path)` the router registers.
const ROUTES: &[(Method, &str)] = &[
    (Method::POST, routes::DECODE),
    (Method::POST, routes::PLACES),
    (Method::GET, routes::NEARBY),
];

/// Whether the router, rather than a handler, produced this response.
///
/// axum answers an unregistered path with an empty `404` and a registered
/// path with the wrong method with `405`. A handler's own `not_found!`
/// carries its message in the body, so it counts as dispatched.
fn router_miss(status: StatusCode, body: &[u8]) -> bool {
    status == StatusCode::METHOD_NOT_ALLOWED || (status == StatusCode::NOT_FOUND && body.is_empty())
}

/// Doubles whose SQL answers no rows and no affected rows, so each
/// table-backed route reaches its handler and answers for itself.
fn provider() -> Provider {
    Provider::default()
        .tables(ScriptedTables::default().on_query(|_, _| true, Vec::new()).on_exec(|_, _| true, 0))
}

async fn send(request: Request<Body>) -> (StatusCode, Vec<u8>) {
    let router = pattern::router().with_state(Client::new("acme", provider()));
    let response = router.oneshot(request).await.expect("response");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    (status, body.to_vec())
}

#[tokio::test]
async fn dispatch() {
    let mut misses = Vec::new();
    for (method, path) in ROUTES {
        let request = Request::builder()
            .method(method.clone())
            .uri(*path)
            .body(Body::empty())
            .expect("request");
        let (status, body) = send(request).await;
        if router_miss(status, &body) {
            misses.push(format!("{method} {path} -> {status}"));
        }
    }
    assert!(misses.is_empty(), "routes not dispatched:\n{}", misses.join("\n"));
}

#[tokio::test]
async fn nearby_body() {
    let request = Request::get(routes::NEARBY)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(&br#"{"lat":-36.8442,"lon":174.7676,"radius_m":500}"#[..]))
        .expect("request");

    let (status, body) = send(request).await;

    assert!(status != StatusCode::BAD_REQUEST && !router_miss(status, &body), "{status}");
}
