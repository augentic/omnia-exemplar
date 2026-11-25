//! Tests for the feature-gated god-mode trip override.
#![cfg(feature = "god-mode")]

#[expect(dead_code, reason = "shared support helpers are used by `static.rs`")]
mod support;

use axum::http::header::CONTENT_TYPE;
use axum::http::{Method, Request, StatusCode};
use bytes::Bytes;
use common::topic;
use gtfs_adapter::set_trip::SetTripReply;
use jiff::Timestamp;
use omnia_test::host::MatchedHttp;
use serde_json::{Value, json};

use self::support::{Adapter, ok, serial_data, trip_instance, trip_instances, trip_query};

fn set_trip() -> Request<Bytes> {
    Request::post("/god-mode/set-trip/EM580/TRIP-9")
        .header(CONTENT_TYPE, "application/json")
        .body(Bytes::from_static(b"{}"))
        .expect("request")
}

#[tokio::test]
async fn set_trip_disabled() {
    // `GOD_MODE_ENABLED` is not configured, so the handler must refuse
    let adapter = Adapter::new(MatchedHttp::default()).await;

    let response = adapter.send(set_trip()).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: Value = serde_json::from_slice(response.body()).expect("error JSON");
    assert!(body["message"].as_str().is_some_and(|msg| msg.contains("God mode not enabled")));
    assert!(adapter.backends.state("god_mode.overrides").await.is_none());
}

#[tokio::test]
async fn set_trip_enabled() {
    let signed_on: Timestamp = "2025-11-20T12:00:00Z".parse().expect("timestamp");

    // only TRIP-9 is scripted, so a lookup of the keyed-in TRIP-1 fails
    let http = MatchedHttp::default().on_matching(
        Method::POST,
        trip_instances(),
        trip_query("TRIP-9", "20251120"),
        ok(json!([trip_instance("TRIP-9")]).to_string()),
    );
    let adapter = Adapter::with_config(http, [("GOD_MODE_ENABLED", "true")]).await;

    // the vehicle is currently on a different trip
    let trip = serde_json::to_vec(&trip_instance("TRIP-0")).expect("serialize");
    adapter.plant("motionGtfs.trip.vehicle.EM580", &trip).await;

    // override
    let response = adapter.send(set_trip()).await;

    assert_eq!(response.status(), StatusCode::OK);
    let reply: SetTripReply = serde_json::from_slice(response.body()).expect("reply");
    assert_eq!(reply.message, "Ok");

    // sign on to another trip
    adapter
        .deliver(topic::PULSE_TO_MOTION, serial_data("EM580", "TRIP-1", signed_on))
        .await
        .expect("should succeed");

    let trip = adapter.backends.state("motionGtfs.trip.vehicle.EM580").await.expect("trip stored");
    let trip: Value = serde_json::from_slice(&trip).expect("should parse trip");
    assert_eq!(trip["tripId"], "TRIP-9");
}
