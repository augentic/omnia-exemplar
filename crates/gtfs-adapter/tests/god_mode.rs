//! Tests for the feature-gated god-mode trip override.
#![cfg(feature = "god-mode")]

#[expect(dead_code, reason = "shared support helpers are used by `static.rs`")]
mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{DateTime, Utc};
use common::topic;
use gtfs_adapter::set_trip::SetTripReply;
use omnia_test::guest::{MatchedHttp, deliver, send};
use serde_json::{Value, json};

use self::support::{ok, provider, router, serial_data, trip_instance, trip_instances, trip_query};

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
    let signed_on: DateTime<Utc> = "2025-11-20T12:00:00Z".parse().expect("timestamp");

    // only TRIP-9 is scripted, so a lookup of the keyed-in TRIP-1 panics
    let base = provider();
    let config = base.config.clone().with([("GOD_MODE_ENABLED", "true")]);
    let provider = base.config(config).http(MatchedHttp::default().on_matching(
        http::Method::POST,
        trip_instances(),
        trip_query("TRIP-9", "20251120"),
        ok(json!([trip_instance("TRIP-9")]).to_string()),
    ));

    // the vehicle is currently on a different trip
    provider.storage.insert_state(
        "motionGtfs:trip:vehicle:EM580",
        &serde_json::to_vec(&trip_instance("TRIP-0")).expect("serialize"),
    );

    // override
    let request = Request::post("/god-mode/set-trip/EM580/TRIP-9")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .expect("request");
    let response = send(&router(&provider), request).await;

    assert_eq!(response.status(), StatusCode::OK);
    let reply: SetTripReply = serde_json::from_slice(response.body()).expect("reply");
    assert_eq!(reply.message, "Ok");

    // sign on to another trip
    deliver(&router(&provider), topic::PULSE_TO_MOTION, serial_data("EM580", "TRIP-1", signed_on))
        .await
        .expect("should succeed");

    let trip = provider.storage.state("motionGtfs:trip:vehicle:EM580").expect("trip stored");
    let trip: Value = serde_json::from_slice(&trip).expect("should parse trip");
    assert_eq!(trip["tripId"], "TRIP-9");
}
