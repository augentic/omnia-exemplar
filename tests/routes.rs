//! Route rung: the production HTTP routing table driven natively.
//!
//! The provider declaration below is `src/lib.rs`'s, differing by the crate
//! path alone; `oneshot` exercises the same `axum::Router` the WASI export
//! serves, so a route, codec, or handler regression surfaces here without a
//! component build.

use acme_common::routes;
use axum::body::{Body, to_bytes};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use omnia_test::guest::MapConfig;
use tally_connector::TallyMessage;
use tower::ServiceExt as _;

omnia_test::provider! {
    /// The production capability list, as doubles.
    pub struct TestProvider: Config + DocumentStore + HttpRequest + Identity + Publish + StateStore
        + TableStore;
}

const TALLY_MESSAGE: &[u8] = include_bytes!("../crates/tally-connector/data/tally-message.json");

fn provider() -> TestProvider {
    TestProvider::default().config(MapConfig::default().with([("ENV", "dev")]))
}

#[tokio::test]
async fn apc_publishes_to_tally_topic() {
    let provider = provider();
    let request = Request::post(routes::http::APC)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(TALLY_MESSAGE))
        .expect("request");

    let response = guest::router(provider.clone()).oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    assert_eq!(body.as_ref(), br#""OK""#);

    let published = provider.publish.sent();
    assert_eq!(published.len(), 1);
    let (topic, record) = &published[0];
    assert_eq!(topic, "dev-realtime-tally-apc.v2");

    let message: TallyMessage = serde_json::from_slice(TALLY_MESSAGE).expect("deserialize");
    let site = message.device.as_ref().expect("device").site.as_str();
    assert_eq!(record.headers.get("key").map(String::as_str), Some(site));
}

#[tokio::test]
async fn unknown_route_is_not_found() {
    let request = Request::get("/nowhere").body(Body::empty()).expect("request");

    let response = guest::router(provider()).oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
