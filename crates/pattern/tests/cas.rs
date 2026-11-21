//! Tests for the `Bucket::update_json` optimistic-concurrency quota update,
//! driven through the stem's component.

mod support;

use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use bytes::Bytes;
use omnia::{Provides, Runtime};
use omnia_test::host::{self, Backends};
use omnia_wasi_http::WasiHttp;
use pattern::cas::QuotaRecord;
use serde_json::{Value, json};
use support::boot;

async fn put_quota<B: Provides<WasiHttp>>(
    runtime: &Runtime<B>, tenant: &str, expected_version: u64, limit: u64,
) -> (StatusCode, Value) {
    let request = Request::put(format!("/examples/patterns/quotas/{tenant}"))
        .header(CONTENT_TYPE, "application/json")
        .body(Bytes::from(
            json!({
                "expected_version": expected_version,
                "limit": limit,
            })
            .to_string(),
        ))
        .expect("request");
    let response = host::send(runtime, request).await.expect("request served");
    assert_eq!(
        response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok()),
        Some("application/json")
    );
    let value = serde_json::from_slice(response.body()).expect("JSON body");
    (response.status(), value)
}

#[tokio::test]
async fn stale_version() {
    let runtime = boot(Backends::defaults().await).await;

    let (status, body) = put_quota(&runtime, "metro", 0, 1_000).await;
    assert_eq!(status, StatusCode::OK);
    let created: QuotaRecord = serde_json::from_value(body).expect("record");
    assert_eq!(
        created,
        QuotaRecord {
            tenant: "metro".to_string(),
            version: 1,
            limit: 1_000,
        }
    );

    let (status, body) = put_quota(&runtime, "metro", 1, 2_500).await;
    assert_eq!(status, StatusCode::OK);
    let updated: QuotaRecord = serde_json::from_value(body).expect("record");
    assert_eq!(
        updated,
        QuotaRecord {
            tenant: "metro".to_string(),
            version: 2,
            limit: 2_500,
        }
    );

    // A caller still holding version 1 is rejected with 409 Conflict
    // naming the stored version to retry with.
    let (status, body) = put_quota(&runtime, "metro", 1, 1_500).await;
    runtime.shutdown();
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        body,
        json!({
            "error": "conflict",
            "message": "quota for `metro` changed concurrently (current version: 2)",
        })
    );
}

#[tokio::test]
async fn non_portable_tenant() {
    let runtime = boot(Backends::defaults().await).await;

    let (status, body) = put_quota(&runtime, "metro:north", 0, 1_000).await;
    runtime.shutdown();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "bad_request");
    assert!(
        body["message"].as_str().is_some_and(|msg| msg.contains("metro:north")),
        "unexpected bad-request message: {body}"
    );
}

#[tokio::test]
async fn missing_quota() {
    let runtime = boot(Backends::defaults().await).await;

    let (status, body) = put_quota(&runtime, "unknown", 1, 500).await;
    runtime.shutdown();
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");
    assert!(
        body["message"].as_str().is_some_and(|msg| msg.contains("unknown")),
        "unexpected not-found message: {body}"
    );
}
