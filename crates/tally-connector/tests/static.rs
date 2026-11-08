//! Static tests for the Tally APC connector driven through its router.

use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use common::topic;
use omnia_sdk::api::Client;
use omnia_test::guest::{Provider, send};
use tally_connector::types::TallyMessage;

const TALLY_MESSAGE: &[u8] = include_bytes!("../data/tally-message.json");
const TALLY_NO_DEVICE: &[u8] = include_bytes!("../data/tally-no-device.json");
const TALLY_WHITESPACE: &[u8] = include_bytes!("../data/tally-whitespace.json");

async fn forward(provider: &Provider, payload: &'static [u8]) {
    let request = Request::post("/api/apc")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(payload))
        .expect("request");
    let router = tally_connector::handlers().with_state(Client::new(provider.clone()));
    let response = send(&router, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.body().as_ref(), br#""OK""#);
}

#[tokio::test]
async fn device_with_site() {
    let provider = Provider::default();

    forward(&provider, TALLY_MESSAGE).await;

    let published = provider.publish.sent();
    assert_eq!(published.len(), 1);

    let (topic, record) = &published[0];
    assert_eq!(topic, topic::TALLY_APC);

    let message: TallyMessage = serde_json::from_slice(TALLY_MESSAGE).expect("should deserialize");

    let expected_key = message.device.as_ref().expect("device").site.as_str();
    assert_eq!(record.headers.get("key").map(String::as_str), Some(expected_key));

    let expected_payload = serde_json::to_vec(&message).expect("should serialize");
    assert_eq!(record.payload, expected_payload);
}

#[tokio::test]
async fn device_missing() {
    let provider = Provider::default();

    forward(&provider, TALLY_NO_DEVICE).await;

    let published = provider.publish.sent();
    assert_eq!(published.len(), 1);

    let (topic, record) = &published[0];
    assert_eq!(topic, topic::TALLY_APC);
    assert_eq!(record.headers.get("key").map(String::as_str), Some("undefined"));

    let message: TallyMessage =
        serde_json::from_slice(TALLY_NO_DEVICE).expect("should deserialize");
    let expected_payload = serde_json::to_vec(&message).expect("should serialize");
    assert_eq!(record.payload, expected_payload);
}

#[tokio::test]
async fn device_site_whitespace() {
    let provider = Provider::default();

    forward(&provider, TALLY_WHITESPACE).await;

    let published = provider.publish.sent();
    assert_eq!(published.len(), 1);

    let (topic, record) = &published[0];
    assert_eq!(topic, topic::TALLY_APC);
    assert_eq!(record.headers.get("key").map(String::as_str), Some("  "));

    let message: TallyMessage =
        serde_json::from_slice(TALLY_WHITESPACE).expect("should deserialize");
    let expected_payload = serde_json::to_vec(&message).expect("should serialize");
    assert_eq!(record.payload, expected_payload);
}
