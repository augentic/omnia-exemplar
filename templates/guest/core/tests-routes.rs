//! Route rung: the routing table driven natively, no component build.
//!
//! `omnia_test::guest::Provider` implements every capability, and the
//! `router` bounds pick out which of its doubles a test seeds; `oneshot`
//! exercises the same `axum::Router` the WASI export serves.

use axum::body::{Body, to_bytes};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use omnia_test::guest::{MapConfig, Provider};
use tower::ServiceExt as _;

#[tokio::test]
async fn greet_uses_configured_greeting() {
    let provider = Provider::default().config(MapConfig::default().with([("GREETING", "Kia ora")]));
    let request = Request::post("/greet")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name":"Omnia"}"#))
        .expect("request");

    let response = <CRATE_NAME>::router(provider).oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    assert_eq!(body.as_ref(), br#"{"message":"Kia ora, Omnia!"}"#);
}
