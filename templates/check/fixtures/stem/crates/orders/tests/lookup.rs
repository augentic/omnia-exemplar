//! Looking up an order through the stem's router, asserting the custom
//! `202 Accepted` status and `409 Conflict` error body.

use axum::body::{Body, to_bytes};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use omnia_sdk::api::Client;
use omnia_test::guest::Provider;
use orders::lookup::OrderStatus;
use serde_json::Value;
use tower::ServiceExt as _;

async fn get_order(provider: &Provider, id: &str) -> (StatusCode, Value) {
    let request = Request::get(format!("/orders/{id}")).body(Body::empty()).expect("request");
    let router = orders::router().with_state(Client::new("test", provider.clone()));
    let response = router.oneshot(request).await.expect("response");
    assert_eq!(
        response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok()),
        Some("application/json")
    );
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    let body = serde_json::from_slice(&bytes).expect("JSON body");
    (status, body)
}

#[tokio::test]
async fn pending_order_is_accepted() {
    let provider = Provider::default();
    let pending = OrderStatus {
        id: "ord-1".to_string(),
        settled: false,
        locked: false,
    };
    provider
        .storage
        .insert_state("orders/order/ord-1", &serde_json::to_vec(&pending).expect("serialize"));

    let (status, body) = get_order(&provider, "ord-1").await;

    assert_eq!(status, StatusCode::ACCEPTED);
    let reply: OrderStatus = serde_json::from_value(body).expect("status");
    assert_eq!(reply, pending);
}

#[tokio::test]
async fn locked_order_is_conflict() {
    let provider = Provider::default();
    let locked = OrderStatus {
        id: "ord-2".to_string(),
        settled: false,
        locked: true,
    };
    provider
        .storage
        .insert_state("orders/order/ord-2", &serde_json::to_vec(&locked).expect("serialize"));

    let (status, body) = get_order(&provider, "ord-2").await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "conflict");
    assert_eq!(body["message"], "order `ord-2` is locked");
    assert_eq!(body["id"], "ord-2");
}
