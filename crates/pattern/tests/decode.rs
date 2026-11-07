//! Tests for the decode-through-cache handler, driven through the stem's
//! router, with the outbound request recorded by `MatchedHttp`.

use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderValue, Method, Request, Response, StatusCode};
use bytes::Bytes;
use omnia_sdk::api::Client;
use omnia_test::guest::{MapConfig, MatchedHttp, Provider, send};
use pattern::decode::{DecodeSegmentReply, Segment};
use serde_json::json;

const DECODER: &str = "https://decoder.test/decode";
#[expect(
    clippy::disallowed_macros,
    reason = "integration test loads captured segment JSON from disk"
)]
const SEGMENT: &[u8] = include_bytes!("../data/segment.json");

fn provider() -> Provider {
    Provider::default()
        .config(
            MapConfig::default().with([
                ("PATTERN_DECODER_URL", DECODER),
                ("PATTERN_CLIENT_CERT", "test-client-cert"),
            ]),
        )
        .http(MatchedHttp::default().on(
            Method::POST,
            DECODER,
            Response::new(Bytes::from_static(SEGMENT)),
        ))
}

async fn decode(provider: &Provider, code: &str) -> DecodeSegmentReply {
    let request = Request::post("/examples/patterns/decode")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "code": code }).to_string()))
        .expect("request");
    let router = pattern::handlers().with_state(Client::new(provider.clone()));
    let response = send(&router, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(response.body()).expect("reply")
}

#[tokio::test]
async fn miss() {
    let provider = provider();

    let reply = decode(&provider, "seg-1").await;

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
        requests[0].headers.get("Client-Cert").map(HeaderValue::as_bytes),
        Some(b"test-client-cert".as_slice())
    );

    // The result was written back through the cache…
    let cached = provider.storage.state("pattern:segment:seg-1").expect("segment cached");
    let segment: Segment = serde_json::from_slice(&cached).expect("valid cached JSON");
    assert_eq!(segment.code, "seg-1");

    // …so a second invocation never reaches HTTP.
    let reply = decode(&provider, "seg-1").await;
    assert!(reply.cached);
    assert_eq!(provider.http.requests().len(), 1);
}

#[tokio::test]
async fn hit() {
    // No config seeded and no route scripted: reading either would fail.
    let provider = Provider::default();
    let segment = json!({ "code": "seg-2", "points": [[0.0, 0.0]] });
    provider
        .storage
        .insert_state("pattern:segment:seg-2", &serde_json::to_vec(&segment).expect("serialize"));

    let reply = decode(&provider, "seg-2").await;

    assert!(reply.cached);
    assert_eq!(reply.segment.code, "seg-2");
    assert!(provider.http.requests().is_empty());
}
