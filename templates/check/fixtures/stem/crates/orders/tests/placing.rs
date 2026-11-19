//! Placing an order through the stem's component, with the line limit
//! configured and every published message recorded.

mod support;

use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use bytes::Bytes;
use common::topic;
use omnia::{Provides, Runtime};
use omnia_test::host::{self, Backends, Sink};
use omnia_wasi_http::WasiHttp;
use orders::placing::Placed;
use support::boot;

async fn place<B: Provides<WasiHttp>>(
    runtime: &Runtime<B>, body: &'static str,
) -> (StatusCode, Bytes) {
    let request = Request::post("/orders")
        .header(CONTENT_TYPE, "application/json")
        .body(Bytes::from_static(body.as_bytes()))
        .expect("request");
    let response = host::send(runtime, request).await.expect("request served");
    (response.status(), response.into_body())
}

#[tokio::test]
async fn place_then_count() {
    let sink = Sink::default();
    let backends =
        Backends::defaults().await.config([("ORDERS_LIMIT", "2")]).messaging(sink.clone());
    let runtime = boot(backends.clone()).await;

    let (status, body) = place(&runtime, r#"{"lines": ["tea", "cake"]}"#).await;

    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let placed: Placed = serde_json::from_slice(&body).expect("a placement");
    assert_eq!(placed.lines, 2);
    let sent = sink.sent();
    assert_eq!(sent.len(), 1, "the placement is published once");
    assert_eq!(sent[0].topic, topic::ORDER_PLACED);
    let carried: Placed = serde_json::from_slice(&sent[0].payload).expect("a placement");
    assert_eq!(carried.lines, 2);

    let (status, _) = place(&runtime, r#"{"lines": ["tea", "cake", "jam"]}"#).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(sink.sent().len(), 1, "a refused order is not published");

    host::deliver(&runtime, topic::ORDER_PLACED, sent[0].payload.clone())
        .await
        .expect("a placement is counted");
    runtime.shutdown();

    let total = backends.state("orders.lines-placed").await;
    assert_eq!(total.as_deref(), Some(b"2".as_slice()), "the placement is counted");
}
