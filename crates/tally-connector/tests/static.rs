//! Static tests for the Tally APC connector driven through its router.

use axum::body::{Body, to_bytes};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use omnia_sdk::api::Client;
use omnia_test::guest::{MapConfig, Provider};
use tally_connector::{APC, TallyMessage};
use tower::ServiceExt as _;

// `config::env` falls back to `dev` when `ENV` is unset; seeding it keeps the
// topic assertions honest rather than leaning on the fallback.
fn provider() -> Provider {
    Provider::default().config(MapConfig::default().with([("ENV", "dev")]))
}

async fn forward(provider: &Provider, payload: &'static [u8]) {
    let request = Request::post(APC)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(payload))
        .expect("request");
    let router = tally_connector::router().with_state(Client::new("acme", provider.clone()));
    let response = router.oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    assert_eq!(body.as_ref(), br#""OK""#);
}

#[tokio::test]
async fn device_with_site() {
    let provider = provider();
    #[expect(
        clippy::disallowed_macros,
        reason = "integration test loads captured Tally JSON from disk"
    )]
    let payload = include_bytes!("../data/tally-message.json");

    forward(&provider, payload).await;

    let published = provider.publish.sent();
    assert_eq!(published.len(), 1);

    let (topic, record) = &published[0];
    assert_eq!(topic, "dev-realtime-tally-apc.v2");

    let message: TallyMessage = serde_json::from_slice(payload).expect("should deserialize");

    let expected_key = message.device.as_ref().expect("device").site.as_str();
    assert_eq!(record.headers.get("key").map(String::as_str), Some(expected_key));

    let expected_payload = serde_json::to_vec(&message).expect("should serialize");
    assert_eq!(record.payload, expected_payload);
}

#[tokio::test]
async fn device_missing() {
    let provider = provider();
    #[expect(
        clippy::disallowed_macros,
        reason = "integration test loads captured Tally JSON from disk"
    )]
    let payload = include_bytes!("../data/tally-no-device.json");

    forward(&provider, payload).await;

    let published = provider.publish.sent();
    assert_eq!(published.len(), 1);

    let (topic, record) = &published[0];
    assert_eq!(topic, "dev-realtime-tally-apc.v2");
    assert_eq!(record.headers.get("key").map(String::as_str), Some("undefined"));

    let message: TallyMessage = serde_json::from_slice(payload).expect("should deserialize");
    let expected_payload = serde_json::to_vec(&message).expect("should serialize");
    assert_eq!(record.payload, expected_payload);
}

#[tokio::test]
async fn device_site_whitespace() {
    let provider = provider();
    #[expect(
        clippy::disallowed_macros,
        reason = "integration test loads captured Tally JSON from disk"
    )]
    let payload = include_bytes!("../data/tally-whitespace.json");

    forward(&provider, payload).await;

    let published = provider.publish.sent();
    assert_eq!(published.len(), 1);

    let (topic, record) = &published[0];
    assert_eq!(topic, "dev-realtime-tally-apc.v2");
    assert_eq!(record.headers.get("key").map(String::as_str), Some("  "));

    let message: TallyMessage = serde_json::from_slice(payload).expect("should deserialize");
    let expected_payload = serde_json::to_vec(&message).expect("should serialize");
    assert_eq!(record.payload, expected_payload);
}
