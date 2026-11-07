//! Route rung: the production router built natively.
//!
//! Building the router is the check: axum panics where two stems merge one
//! path under one method. A stem's routes are driven by its own tests
//! through its `handlers()`; this rung holds the merged whole.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use omnia_test::guest::{Provider, send};

#[test]
fn merges() {
    let _ = guest::router(Provider::default());
}

#[tokio::test]
async fn unknown_path() {
    let request = Request::get("/nothing-here").body(Body::empty()).expect("request");

    let response = send(&guest::router(Provider::default()), request).await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
