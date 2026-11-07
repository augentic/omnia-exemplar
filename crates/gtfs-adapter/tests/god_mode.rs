//! Tests for the feature-gated god-mode trip override.
#![cfg(feature = "god-mode")]

#[expect(dead_code, reason = "shared support helpers are used by `static.rs`")]
mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use gtfs_adapter::set_trip::SetTripReply;
use omnia_test::guest::{MapConfig, send};
use serde_json::Value;

use self::support::{provider, router};

#[tokio::test]
async fn set_trip_disabled() {
    // `GOD_MODE_ENABLED` is not configured, so the handler must refuse
    let provider = provider();
    let request = Request::post("/god-mode/set-trip/EM580/TRIP-9")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .expect("request");

    let response = send(&router(&provider), request).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: Value = serde_json::from_slice(response.body()).expect("error JSON");
    assert!(body["message"].as_str().is_some_and(|msg| msg.contains("God mode not enabled")));
    assert!(provider.storage.state("god_mode:overrides").is_none());
}

#[tokio::test]
async fn set_trip_enabled() {
    let provider = provider().config(MapConfig::default().with([("GOD_MODE_ENABLED", "true")]));
    let request = Request::post("/god-mode/set-trip/EM580/TRIP-9")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .expect("request");

    let response = send(&router(&provider), request).await;

    assert_eq!(response.status(), StatusCode::OK);
    let reply: SetTripReply = serde_json::from_slice(response.body()).expect("reply");
    assert_eq!(reply.message, "Ok");
    assert_eq!(reply.process, 0);

    let state = provider.storage.state("god_mode:overrides").expect("override should be stored");
    let state: Value = serde_json::from_slice(&state).expect("should parse state");
    assert_eq!(state["overrides"]["EM580"], "TRIP-9");
}
