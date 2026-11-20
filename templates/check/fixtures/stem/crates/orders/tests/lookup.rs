//! Looking up an order through the stem's component, asserting the custom
//! `202 Accepted` status and `409 Conflict` error body.

mod support;

use axum::http::{Request, StatusCode};
use bytes::Bytes;
use omnia::{Provides, Runtime};
use omnia_test::host::{self, Backends, STATE_BUCKET};
use omnia_wasi_http::WasiHttp;
use omnia_wasi_keyvalue::{KeyValueClient as _, Lifetime};
use orders::lookup::OrderStatus;
use serde_json::Value;
use support::boot;

async fn get_order<B: Provides<WasiHttp>>(runtime: &Runtime<B>, id: &str) -> (StatusCode, Value) {
    let request = Request::get(format!("/orders/{id}")).body(Bytes::new()).expect("request");
    let response = host::send(runtime, request).await.expect("request served");
    let body = serde_json::from_slice(response.body()).expect("JSON body");
    (response.status(), body)
}

async fn store(backends: &Backends, order: &OrderStatus) {
    let bucket = backends.keyvalue.open_bucket(STATE_BUCKET.to_owned()).await.expect("bucket");
    let value = serde_json::to_vec(order).expect("serialize");
    bucket
        .set(format!("orders.order.{}", order.id), value, Lifetime::Replace(None), None)
        .await
        .expect("order stored")
        .expect("no precondition to fail");
}

#[tokio::test]
async fn lookup() {
    let backends = Backends::defaults().await;
    let runtime = boot(backends.clone()).await;
    let mut order = OrderStatus {
        id: "ord-1".to_string(),
        settled: false,
        locked: false,
    };
    store(&backends, &order).await;

    let (status, body) = get_order(&runtime, "ord-1").await;

    assert_eq!(status, StatusCode::ACCEPTED);
    let reply: OrderStatus = serde_json::from_value(body).expect("status");
    assert_eq!(reply, order);

    order.locked = true;
    store(&backends, &order).await;

    let (status, body) = get_order(&runtime, "ord-1").await;
    runtime.shutdown();

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "conflict");
    assert_eq!(body["message"], "order `ord-1` is locked");
    assert_eq!(body["id"], "ord-1");
}
