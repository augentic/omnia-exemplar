//! Static tests for the Tally APC connector.

use omnia_guest::api::{Client, Metadata};
use omnia_test::guest::MapConfig;
use tally_connector::{TallyMessage, TallyRequest};

omnia_test::provider! {
    /// The handler's capability pair, as doubles.
    pub struct TestProvider: Config + Publish;
}

// `config::env` falls back to `dev` when `ENV` is unset; seeding it keeps the
// topic assertions honest rather than leaning on the fallback.
fn provider() -> TestProvider {
    TestProvider::default().config(MapConfig::default().with([("ENV", "dev")]))
}

async fn forward(provider: &TestProvider, payload: &[u8]) {
    let request: TallyRequest = serde_json::from_slice(payload).expect("should deserialize");
    Client::new("acme", provider.clone())
        .call(request, &Metadata::default())
        .await
        .expect("should succeed");
}

#[tokio::test]
async fn device_site_header() {
    let provider = provider();
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
