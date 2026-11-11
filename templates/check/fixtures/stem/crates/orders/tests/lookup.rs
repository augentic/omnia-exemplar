//! Looking up an order through the stem's router, asserting the custom
//! `202 Accepted` status and `409 Conflict` error body.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use omnia_sdk::api::Client;
use omnia_test::guest::{Provider, send};
use orders::lookup::OrderStatus;
use serde_json::Value;

async fn get_order(provider: &Provider, id: &str) -> (StatusCode, Value) {
    let request = Request::get(format!("/orders/{id}")).body(Body::empty()).expect("request");
    let router = orders::handlers().with_state(Client::new(provider.clone()));
    let response = send(&router, request).await;
    let body = serde_json::from_slice(response.body()).expect("JSON body");
    (response.status(), body)
}

#[tokio::test]
async fn lookup() {
    let provider = Provider::default();
    let mut order = OrderStatus {
        id: "ord-1".to_string(),
        settled: false,
        locked: false,
    };
    provider
        .storage
        .insert_state("orders/order/ord-1", &serde_json::to_vec(&order).expect("serialize"));

    let (status, body) = get_order(&provider, "ord-1").await;

    assert_eq!(status, StatusCode::ACCEPTED);
    let reply: OrderStatus = serde_json::from_value(body).expect("status");
    assert_eq!(reply, order);

    order.locked = true;
    provider
        .storage
        .insert_state("orders/order/ord-1", &serde_json::to_vec(&order).expect("serialize"));

    let (status, body) = get_order(&provider, "ord-1").await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "conflict");
    assert_eq!(body["message"], "order `ord-1` is locked");
    assert_eq!(body["id"], "ord-1");
}
