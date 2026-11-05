//! Route rung: the production router built natively.
//!
//! Building the router is the check: axum panics where two stems merge one
//! path under one method. A stem's routes are driven by its own tests
//! through its `routes()`; this rung holds the merged whole.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::config;
use omnia_test::guest::{MapConfig, Provider};
use tower::ServiceExt as _;

fn provider() -> Provider {
    Provider::default().config(MapConfig::default().with([(config::ENV, "test")]))
}

#[test]
fn merges() {
    let _ = guest::http_router(provider());
}

#[tokio::test]
async fn an_unknown_path_is_not_found() {
    let request = Request::get("/nothing-here").body(Body::empty()).expect("request");

    let response = guest::http_router(provider()).oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
