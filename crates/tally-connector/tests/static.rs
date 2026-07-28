//! Static tests for the Tally APC connector.

mod provider;

use omnia_guest::api::{Invocation, Invoker};
use tally_connector::{TallyMessage, TallyRequest};

use self::provider::MockProvider;

async fn forward(provider: &MockProvider, payload: &[u8]) {
    let request: TallyRequest = serde_json::from_slice(payload).expect("should deserialize");
    Invoker::new("acme", provider.clone())
        .invoke::<TallyRequest>(Invocation::new(request))
        .await
        .expect("should succeed");
}

#[tokio::test]
async fn device_site_header() {
    let provider = MockProvider::default();
    let payload = include_bytes!("../data/tally-message.json");

    forward(&provider, payload).await;

    let published = provider.published();
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
    let provider = MockProvider::default();
    let payload = include_bytes!("../data/tally-no-device.json");

    forward(&provider, payload).await;

    let published = provider.published();
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
    let provider = MockProvider::default();
    let payload = include_bytes!("../data/tally-whitespace.json");

    forward(&provider, payload).await;

    let published = provider.published();
    assert_eq!(published.len(), 1);

    let (topic, record) = &published[0];
    assert_eq!(topic, "dev-realtime-tally-apc.v2");
    assert_eq!(record.headers.get("key").map(String::as_str), Some("  "));

    let message: TallyMessage = serde_json::from_slice(payload).expect("should deserialize");
    let expected_payload = serde_json::to_vec(&message).expect("should serialize");
    assert_eq!(record.payload, expected_payload);
}
