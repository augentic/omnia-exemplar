//! Placing an order through the stem's routers, over the guest doubles.

use axum::body::{Body, to_bytes};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use common::config;
use omnia_sdk::StateStore;
use omnia_sdk::api::Client;
use omnia_sdk::api::messaging::{Delivery, Router};
use omnia_test::guest::{MapConfig, Provider};
use orders::placing::Placed;
use tower::ServiceExt as _;

fn provider() -> Provider {
    Provider::default()
        .config(MapConfig::default().with([(config::ENV, "test"), ("ORDERS_LIMIT", "2")]))
}

async fn place(provider: &Provider, body: &'static str) -> (StatusCode, Vec<u8>) {
    let request = Request::post("/orders")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .expect("request");
    let router = orders::router().with_state(Client::new("test", provider.clone()));
    let response = router.oneshot(request).await.expect("response");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    (status, body.to_vec())
}

#[tokio::test]
async fn placed_within_the_limit() {
    let provider = provider();

    let (status, body) = place(&provider, r#"{"lines": ["tea", "cake"]}"#).await;

    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let placed: Placed = serde_json::from_slice(&body).expect("a placement");
    assert_eq!(placed.lines, 2);
    let sent = provider.publish.sent();
    assert_eq!(sent.len(), 1, "the placement is published once");
    assert_eq!(sent[0].0, "test-order-placed");
    let carried: Placed = serde_json::from_slice(&sent[0].1.payload).expect("a placement");
    assert_eq!(carried.lines, 2);
}

#[tokio::test]
async fn refused_past_the_limit() {
    let provider = provider();

    let (status, _) = place(&provider, r#"{"lines": ["tea", "cake", "jam"]}"#).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(provider.publish.sent().is_empty(), "nothing is published");
}

#[tokio::test]
async fn placements_counted() {
    let provider = provider();
    let router = orders::topics(Router::new(Client::new("test", provider.clone())), "test");

    for lines in [1, 2] {
        let delivery = Delivery {
            topic: Some("test-order-placed".to_string()),
            payload: serde_json::to_vec(&Placed { lines }).expect("a placement"),
            ..Delivery::default()
        };
        router.handle(delivery).await.expect("a placement is counted");
    }

    let total = StateStore::increment(&provider, "orders/lines-placed", 0).await.expect("the count");
    assert_eq!(total, 3, "the placements are counted");
}
