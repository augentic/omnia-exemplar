//! Tests for the `IdentityExt::access_token` upstream authentication
//! vignette, driven through the stem's router.

use axum::body::Body;
use axum::http::header::AUTHORIZATION;
use axum::http::{Method, Request, Response, StatusCode};
use bytes::Bytes;
use omnia_sdk::api::Client;
use omnia_test::guest::{FixedIdentity, MapConfig, MatchedHttp, Provider, send};
use serde_json::json;

const UPSTREAM_BASE: &str = "https://upstream.test";

#[tokio::test]
async fn upstream_status() {
    let payload = json!({ "resource": "lines", "status": "healthy" }).to_string();
    let provider = Provider::default()
        .config(MapConfig::default().with([
            ("PATTERN_UPSTREAM_URL", UPSTREAM_BASE),
            ("PATTERN_UPSTREAM_IDENTITY", "transit-api"),
        ]))
        .identity(FixedIdentity::new("token-abc-123"))
        .http(MatchedHttp::default().on(
            Method::GET,
            "https://upstream.test/status/lines",
            Response::new(Bytes::from(payload)),
        ));

    let request =
        Request::get("/examples/patterns/upstream/lines").body(Body::empty()).expect("request");
    let router = pattern::handlers().with_state(Client::new(provider.clone()));
    let response = send(&router, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let requests = provider.http.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].headers.get(AUTHORIZATION).and_then(|value| value.to_str().ok()),
        Some("Bearer token-abc-123:transit-api")
    );
}
