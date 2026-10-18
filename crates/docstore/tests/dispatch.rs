//! Every route the crate registers reaches its handler through the router,
//! each method a `.merge()` chain adds to one path included.

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use docstore::paths;
use omnia_sdk::api::Client;
use omnia_test::guest::Provider;
use tower::ServiceExt as _;

/// Every `(method, path)` the router registers.
const ROUTES: &[(Method, &str)] = &[
    (Method::GET, paths::STOPS),
    (Method::POST, paths::STOPS),
    (Method::GET, paths::STOP),
    (Method::PUT, paths::STOP),
    (Method::DELETE, paths::STOP),
    (Method::GET, paths::ROUTES),
    (Method::POST, paths::ROUTES),
    (Method::GET, paths::ROUTE),
    (Method::GET, paths::STOP_TIMES),
    (Method::POST, paths::STOP_TIMES),
    (Method::GET, paths::STOP_TIME),
];

/// Replace each `{param}` segment of a route pattern with a concrete id.
fn concrete(path: &str) -> String {
    path.split('/')
        .map(|segment| if segment.starts_with('{') { "1" } else { segment })
        .collect::<Vec<_>>()
        .join("/")
}

/// Whether the router, rather than a handler, produced this response.
///
/// axum answers an unregistered path with an empty `404` and a registered
/// path with the wrong method with `405`. A handler's own `not_found!`
/// carries its message in the body, so it counts as dispatched.
fn router_miss(status: StatusCode, body: &[u8]) -> bool {
    status == StatusCode::METHOD_NOT_ALLOWED || (status == StatusCode::NOT_FOUND && body.is_empty())
}

#[tokio::test]
async fn dispatch() {
    let provider = Provider::default();
    let mut misses = Vec::new();
    for (method, pattern) in ROUTES {
        let path = concrete(pattern);
        let request = Request::builder()
            .method(method.clone())
            .uri(&path)
            .body(Body::empty())
            .expect("request");
        let router = docstore::router().with_state(Client::new("acme", provider.clone()));
        let response = router.oneshot(request).await.expect("response");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
        if router_miss(status, &body) {
            misses.push(format!("{method} {path} -> {status}"));
        }
    }
    assert!(misses.is_empty(), "routes not dispatched:\n{}", misses.join("\n"));
}
