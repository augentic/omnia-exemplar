//! Tests for the `StateStore::cas` optimistic-concurrency quota update,
//! driven through the stem's router.

use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use omnia_sdk::api::Client;
use omnia_test::guest::{Provider, send};
use pattern::cas::QuotaRecord;
use serde_json::{Value, json};

async fn put_quota(
    provider: &Provider, tenant: &str, expected_version: u64, limit: u64,
) -> (StatusCode, Value) {
    let request = Request::put(format!("/examples/patterns/quotas/{tenant}"))
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "expected_version": expected_version,
                "limit": limit,
            })
            .to_string(),
        ))
        .expect("request");
    let router = pattern::handlers().with_state(Client::new(provider.clone()));
    let response = send(&router, request).await;
    assert_eq!(
        response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok()),
        Some("application/json")
    );
    let value = serde_json::from_slice(response.body()).expect("JSON body");
    (response.status(), value)
}

#[tokio::test]
async fn stale_version() {
    let provider = Provider::default();

    let (status, body) = put_quota(&provider, "metro", 0, 1_000).await;
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

    let (status, body) = put_quota(&provider, "metro", 1, 2_500).await;
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
    // carrying the stored version to retry with.
    let (status, body) = put_quota(&provider, "metro", 1, 1_500).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "conflict");
    assert!(
        body["message"].as_str().is_some_and(|msg| msg.contains("metro")),
        "unexpected conflict message: {body}"
    );
    assert_eq!(body["tenant"], "metro");
    assert_eq!(body["current_version"], 2);
}

#[tokio::test]
async fn missing_quota() {
    let provider = Provider::default();

    let (status, body) = put_quota(&provider, "unknown", 1, 500).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");
    assert!(
        body["message"].as_str().is_some_and(|msg| msg.contains("unknown")),
        "unexpected not-found message: {body}"
    );
}
