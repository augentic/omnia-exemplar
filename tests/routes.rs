//! Route rung: the production router built natively.
//!
//! Building the router is the check: axum panics where two crates merge one
//! path under one method. A crate's routes are driven by its own `dispatch`
//! tests through its `router()`; this rung holds the merged whole.

use acme_common::config;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use omnia_test::guest::{MapConfig, Provider};
use tower::ServiceExt as _;

fn provider() -> Provider {
    Provider::default().config(MapConfig::default().with([(config::ENV, "dev")]))
}

#[tokio::test]
async fn merges() {
    let request = Request::get("/").body(Body::empty()).expect("request");

    let response = guest::router(provider()).oneshot(request).await.expect("response");

    assert_ne!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn an_unknown_path_is_not_found() {
    let request = Request::get("/nothing-here").body(Body::empty()).expect("request");

    let response = guest::router(provider()).oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
