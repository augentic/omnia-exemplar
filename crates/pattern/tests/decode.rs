//! Tests for the decode-through-cache handler, driven through the stem's
//! component, with the outbound request recorded by `MatchedHttp`.

mod support;

use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderValue, Method, Request, Response, StatusCode};
use bytes::Bytes;
use omnia::{Provides, Runtime};
use omnia_test::host::{self, Backends, MatchedHttp, STATE_BUCKET};
use omnia_wasi_http::WasiHttp;
use omnia_wasi_keyvalue::{KeyValueClient as _, Lifetime};
use pattern::decode::{DecodeSegmentReply, Segment};
use serde_json::json;
use support::boot;

const DECODER: &str = "https://decoder.test/decode";
const SEGMENT: &[u8] = include_bytes!("../data/segment.json");

async fn post<B: Provides<WasiHttp>>(runtime: &Runtime<B>, code: &str) -> Response<Bytes> {
    let request = Request::post("/examples/patterns/decode")
        .header(CONTENT_TYPE, "application/json")
        .body(Bytes::from(json!({ "code": code }).to_string()))
        .expect("request");
    host::send(runtime, request).await.expect("request served")
}

async fn decode<B: Provides<WasiHttp>>(runtime: &Runtime<B>, code: &str) -> DecodeSegmentReply {
    let response = post(runtime, code).await;
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(response.body()).expect("reply")
}

#[tokio::test]
async fn miss() {
    let http = MatchedHttp::default().on(
        Method::POST,
        DECODER,
        Response::new(Bytes::from_static(SEGMENT)),
    );
    let backends = Backends::defaults()
        .await
        .config([("PATTERN_DECODER_URL", DECODER), ("PATTERN_CLIENT_CERT", "test-client-cert")])
        .http(http.clone());
    let runtime = boot(backends.clone()).await;

    let reply = decode(&runtime, "seg-1").await;
    runtime.shutdown();
    assert!(!reply.cached);

    // One decoder call, with the certificate from config riding the
    // Client-Cert header.
    let requests = http.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].headers.get("Client-Cert").map(HeaderValue::as_bytes),
        Some(b"test-client-cert".as_slice())
    );

    // The decoded segment is written back under the code's cache key.
    let cached = backends.state("pattern.segment.seg-1").await.expect("segment cached");
    let _: Segment = serde_json::from_slice(&cached).expect("valid cached JSON");
}

#[tokio::test]
async fn hit() {
    // No config seeded and no route scripted: reading either would fail.
    let http = MatchedHttp::default();
    let backends = Backends::defaults().await.http(http.clone());
    let segment = json!({ "code": "seg-2", "points": [[0.0, 0.0]] });
    let bucket =
        backends.keyvalue.open_bucket(STATE_BUCKET.to_owned()).await.expect("bucket opens");
    bucket
        .set(
            "pattern.segment.seg-2".to_owned(),
            serde_json::to_vec(&segment).expect("serialize"),
            Lifetime::Replace(None),
            None,
        )
        .await
        .expect("segment stored")
        .expect("no precondition to fail");
    let runtime = boot(backends).await;

    let reply = decode(&runtime, "seg-2").await;
    runtime.shutdown();

    assert!(reply.cached);
    assert_eq!(reply.segment.code, "seg-2");
    assert!(http.requests().is_empty());
}

// No config seeded and no route scripted: a miss past the check would fail.
#[tokio::test]
async fn non_portable_code() {
    let runtime = boot(Backends::defaults().await.http(MatchedHttp::default())).await;

    let response = post(&runtime, "seg:3").await;
    runtime.shutdown();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = serde_json::from_slice(response.body()).expect("JSON body");
    assert_eq!(body["error"], "bad_request");
}
