//! Every route the crate registers reaches its handler through the router,
//! each method a `.merge()` chain adds to one path included.

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use omnia_sdk::api::Client;
use omnia_test::guest::{Provider, ScriptedTables};
use sql::paths;
use tower::ServiceExt as _;

/// Every `(method, path)` the router registers.
const ROUTES: &[(Method, &str)] = &[
    (Method::GET, paths::AGENCIES),
    (Method::POST, paths::AGENCIES),
    (Method::GET, paths::AGENCY),
    (Method::PATCH, paths::AGENCY),
    (Method::GET, paths::AGENCY_FEEDS),
    (Method::POST, paths::AGENCY_FEEDS),
    (Method::GET, paths::FEEDS),
    (Method::DELETE, paths::FEED),
];

/// Doubles whose SQL answers no rows and no affected rows, so each
/// table-backed route reaches its handler and answers for itself.
fn provider() -> Provider {
    Provider::default()
        .tables(ScriptedTables::default().on_query(|_, _| true, Vec::new()).on_exec(|_, _| true, 0))
}

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
    let provider = provider();
    let mut misses = Vec::new();
    for (method, pattern) in ROUTES {
        let path = concrete(pattern);
        let request = Request::builder()
            .method(method.clone())
            .uri(&path)
            .body(Body::empty())
            .expect("request");
        let router = sql::router().with_state(Client::new("acme", provider.clone()));
        let response = router.oneshot(request).await.expect("response");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
        if router_miss(status, &body) {
            misses.push(format!("{method} {path} -> {status}"));
        }
    }
    assert!(misses.is_empty(), "routes not dispatched:\n{}", misses.join("\n"));
}
