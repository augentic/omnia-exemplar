//! Route rung: the production router built natively.
//!
//! Building the router is the check: axum panics where two stems merge one
//! path under one method. A stem's routes are driven by its own suites
//! through its component; this rung holds the merged whole.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt as _;

#[test]
fn merges() {
    let _ = guest::router();
}

#[tokio::test]
async fn unknown_path() {
    let request = Request::get("/nothing-here").body(Body::empty()).expect("request");

    let router = axum::Router::from(guest::router());
    let response = router.oneshot(request).await.expect("infallible");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
