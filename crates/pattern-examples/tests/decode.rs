//! Tests for the decode-through-cache handler, driven exactly as the guest
//! invokes it, with the outbound request recorded by `MatchedHttp`.

use bytes::Bytes;
use http::{Method, Response};
use omnia_guest::api::{Client, Metadata};
use omnia_test::guest::{MapConfig, MatchedHttp};
use pattern_examples::Segment;
use pattern_examples::decode::{CLIENT_CERT, DECODER_URL, DecodeSegmentRequest, segment_key};

omnia_test::provider! {
    /// The handler's capability list, as doubles.
    pub struct TestProvider: Config + HttpRequest + StateStore;
}

const DECODER: &str = "https://decoder.test/decode";

/// A provider whose decoder answers with the fixture segment.
fn provider() -> TestProvider {
    let segment = Bytes::from_static(include_bytes!("../data/segment.json"));
    TestProvider::default()
        .config(
            MapConfig::default().with([(DECODER_URL, DECODER), (CLIENT_CERT, "test-client-cert")]),
        )
        .http(MatchedHttp::default().on(Method::POST, DECODER, Response::new(segment)))
}

#[tokio::test]
async fn miss_fetches_with_cert_and_caches() {
    let provider = provider();
    let client = Client::new("acme", provider.clone());

    let request = DecodeSegmentRequest {
        code: "seg-1".to_string(),
    };
    let reply = client.call(request, &Metadata::default()).await.expect("should succeed");

    assert!(!reply.cached);
    assert_eq!(reply.segment.code, "seg-1");
    assert_eq!(reply.segment.points.len(), 2);

    // The recording proves the outbound request shape: exactly one call,
    // with the certificate from config riding the Client-Cert header.
    let requests = provider.http.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, Method::POST);
    assert_eq!(requests[0].uri.path(), "/decode");
    assert_eq!(
        requests[0].headers.get("Client-Cert").map(http::HeaderValue::as_bytes),
        Some(b"test-client-cert".as_slice())
    );

    // The result was written back through the cache…
    let cached = provider.storage.state(&segment_key("seg-1")).expect("segment cached");
    let segment: Segment = serde_json::from_slice(&cached).expect("valid cached JSON");
    assert_eq!(segment.code, "seg-1");

    // …so a second invocation never reaches HTTP.
    let request = DecodeSegmentRequest {
        code: "seg-1".to_string(),
    };
    let reply = client.call(request, &Metadata::default()).await.expect("should succeed");
    assert!(reply.cached);
    assert_eq!(provider.http.requests().len(), 1);
}

#[tokio::test]
async fn hit_skips_config_and_http() {
    // No config seeded and no route scripted: reading either would fail.
    let provider = TestProvider::default();
    let segment = serde_json::json!({ "code": "seg-2", "points": [[0.0, 0.0]] });
    provider
        .storage
        .insert_state(&segment_key("seg-2"), &serde_json::to_vec(&segment).expect("serialize"));

    let request = DecodeSegmentRequest {
        code: "seg-2".to_string(),
    };
    let reply = Client::new("acme", provider.clone())
        .call(request, &Metadata::default())
        .await
        .expect("should succeed");

    assert!(reply.cached);
    assert_eq!(reply.segment.code, "seg-2");
    assert!(provider.http.requests().is_empty());
}
