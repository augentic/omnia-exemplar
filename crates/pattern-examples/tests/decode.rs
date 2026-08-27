//! Tests for the decode-through-cache handler, driven through the spy
//! mock provider exactly as the guest invokes it.

mod provider;

use omnia_guest::api::{Client, Metadata};
use pattern_examples::Segment;
use pattern_examples::decode::{DecodeSegmentRequest, segment_key};

use self::provider::MockProvider;

#[tokio::test]
async fn miss_fetches_with_cert_and_caches() {
    let provider = MockProvider::default();
    let client = Client::new("acme", provider.clone());

    let request = DecodeSegmentRequest {
        code: "seg-1".to_string(),
    };
    let reply = client.call(request, &Metadata::default()).await.expect("should succeed");

    assert!(!reply.cached);
    assert_eq!(reply.segment.code, "seg-1");
    assert_eq!(reply.segment.points.len(), 2);

    // The spy proves the outbound request shape: exactly one call, with the
    // certificate from config riding the Client-Cert header.
    let requests = provider.requests_for("/decode");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].client_cert.as_deref(), Some("test-client-cert"));

    // The result was written back through the cache…
    let cached = provider.state(&segment_key("seg-1")).expect("segment cached");
    let segment: Segment = serde_json::from_slice(&cached).expect("valid cached JSON");
    assert_eq!(segment.code, "seg-1");

    // …so a second invocation never reaches HTTP.
    let request = DecodeSegmentRequest {
        code: "seg-1".to_string(),
    };
    let reply = client.call(request, &Metadata::default()).await.expect("should succeed");
    assert!(reply.cached);
    assert_eq!(provider.requests().len(), 1);
}

#[tokio::test]
async fn hit_skips_config_and_http() {
    let provider = MockProvider::default();
    let segment = serde_json::json!({ "code": "seg-2", "points": [[0.0, 0.0]] });
    provider.seed_state(&segment_key("seg-2"), serde_json::to_vec(&segment).expect("serialize"));

    let request = DecodeSegmentRequest {
        code: "seg-2".to_string(),
    };
    let reply = Client::new("acme", provider.clone())
        .call(request, &Metadata::default())
        .await
        .expect("should succeed");

    assert!(reply.cached);
    assert_eq!(reply.segment.code, "seg-2");
    assert!(provider.requests().is_empty());
}
