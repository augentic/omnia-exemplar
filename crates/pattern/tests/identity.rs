//! Tests for the `Identity::access_token` upstream authentication vignette,
//! driven through the stem's router.

use axum::body::{Body, to_bytes};
use axum::http::header::AUTHORIZATION;
use axum::http::{Method, Request, Response, StatusCode};
use bytes::Bytes;
use omnia_sdk::api::Client;
use omnia_test::guest::{FixedIdentity, MapConfig, MatchedHttp, Provider};
use pattern::identity::UpstreamStatusReply;
use serde_json::{Value, json};
use tower::ServiceExt as _;

const UPSTREAM_BASE: &str = "https://upstream.test";

#[tokio::test]
async fn forwards_bearer_token() {
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
    let response = router.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    let reply: UpstreamStatusReply = serde_json::from_slice(&bytes).expect("reply");
    assert_eq!(
        reply,
        UpstreamStatusReply {
            resource: "lines".to_string(),
            status: "healthy".to_string(),
        }
    );

    let requests = provider.http.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].headers.get(AUTHORIZATION).and_then(|value| value.to_str().ok()),
        Some("Bearer token-abc-123")
    );
}

#[tokio::test]
async fn upstream_failure_is_bad_gateway() {
    let provider = Provider::default()
        .config(MapConfig::default().with([
            ("PATTERN_UPSTREAM_URL", UPSTREAM_BASE),
            ("PATTERN_UPSTREAM_IDENTITY", "transit-api"),
        ]))
        .identity(FixedIdentity::new("token-abc-123"))
        .http(
            MatchedHttp::default().on(
                Method::GET,
                "https://upstream.test/status/lines",
                Response::builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .body(Bytes::new())
                    .expect("response"),
            ),
        );

    let request =
        Request::get("/examples/patterns/upstream/lines").body(Body::empty()).expect("request");
    let router = pattern::handlers().with_state(Client::new(provider));
    let response = router.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    let body: Value = serde_json::from_slice(&bytes).expect("error JSON");
    assert_eq!(body["error"], "bad_gateway");
}
