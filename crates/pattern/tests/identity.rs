//! Tests for the `Identity::access_token` upstream authentication vignette,
//! driven through the stem's component.

mod support;

use axum::http::header::AUTHORIZATION;
use axum::http::{Method, Request, Response, StatusCode};
use bytes::Bytes;
use omnia_test::host::{self, Backends, FixedIdentity, MatchedHttp};
use serde_json::json;
use support::boot;

const UPSTREAM_BASE: &str = "https://upstream.test";

#[tokio::test]
async fn upstream_status() {
    let payload = json!({ "resource": "lines", "status": "healthy" }).to_string();
    let http = MatchedHttp::default().on(
        Method::GET,
        "https://upstream.test/status/lines",
        Response::new(Bytes::from(payload)),
    );
    let backends = Backends::defaults()
        .await
        .config([
            ("PATTERN_UPSTREAM_URL", UPSTREAM_BASE),
            ("PATTERN_UPSTREAM_IDENTITY", "transit-api"),
        ])
        .identity(FixedIdentity::new("token-abc-123"))
        .http(http.clone());
    let runtime = boot(backends).await;

    let request =
        Request::get("/examples/patterns/upstream/lines").body(Bytes::new()).expect("request");
    let response = host::send(&runtime, request).await.expect("request served");
    runtime.shutdown();
    assert_eq!(response.status(), StatusCode::OK);

    let requests = http.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].headers.get(AUTHORIZATION).and_then(|value| value.to_str().ok()),
        Some("Bearer token-abc-123:transit-api")
    );
}
