//! Tests for the feature-gated god-mode trip override.
#![cfg(feature = "god-mode")]

#[expect(dead_code, reason = "shared support helpers are used by `static.rs`")]
mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use common::config;
use gtfs_adapter::set_trip::SetTripReply;
use omnia_sdk::api::Client;
use omnia_test::guest::{MapConfig, Provider};
use serde_json::Value;
use tower::ServiceExt as _;

use self::support::provider;

async fn send(provider: Provider, request: Request<Body>) -> (StatusCode, Value) {
    let router = gtfs_adapter::routes().with_state(Client::new(provider));
    let response = router.oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

#[tokio::test]
async fn set_trip_disabled() {
    // `GOD_MODE_ENABLED` is not configured, so the handler must refuse
    let provider = provider();
    let request = Request::post("/god-mode/set-trip/EM580/TRIP-9")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .expect("request");

    let (status, body) = send(provider.clone(), request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["message"].as_str().is_some_and(|msg| msg.contains("God mode not enabled")));
    assert!(provider.storage.state("god_mode:overrides").is_none());
}

#[tokio::test]
async fn set_trip_enabled() {
    let provider = provider()
        .config(MapConfig::default().with([(config::ENV, "dev"), ("GOD_MODE_ENABLED", "true")]));
    let request = Request::post("/god-mode/set-trip/EM580/TRIP-9")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .expect("request");

    let (status, body) = send(provider.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let reply: SetTripReply = serde_json::from_value(body).expect("reply");
    assert_eq!(reply.message, "Ok");
    assert_eq!(reply.process, 0);

    let state = provider.storage.state("god_mode:overrides").expect("override should be stored");
    let state: Value = serde_json::from_slice(&state).expect("should parse state");
    assert_eq!(state["overrides"]["EM580"], "TRIP-9");
}
