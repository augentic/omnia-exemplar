//! Route rung: the production HTTP routing table driven natively.
//!
//! `omnia_test::guest::Provider` implements every capability, and `oneshot`
//! exercises the same `axum::Router` the WASI export serves, so a route
//! regression surfaces here without a component build. `dispatch` checks
//! wiring, not semantics: every `(method, path)` the router registers must
//! reach its handler. Business logic is covered by each stem's own tests.

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use common::config;
use omnia_test::guest::{MapConfig, Provider, ScriptedTables};
use tower::ServiceExt as _;

/// Every `(method, path)` the router registers, one entry per route of
/// `src/routes.rs`, over the constants of `common::routes::http` spelled in full.
const ROUTES: &[(Method, &str)] = &[
    // one entry per route, each added at the end
];

/// Default doubles with the one config key every handler may read.
///
/// `ScriptedTables` panics on an unscripted statement, so SQL is answered
/// with no rows and no affected rows: table-backed routes reach their
/// handler and answer for themselves instead of aborting the request.
fn provider() -> Provider {
    Provider::default()
        .config(MapConfig::default().with([(config::ENV, "test")]))
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
        let response = guest::router(provider.clone()).oneshot(request).await.expect("response");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
        if router_miss(status, &body) {
            misses.push(format!("{method} {path} -> {status}"));
        }
    }
    assert!(misses.is_empty(), "routes not dispatched:\n{}", misses.join("\n"));
}

#[tokio::test]
async fn an_unknown_path_is_not_found() {
    let request = Request::get("/nothing-here").body(Body::empty()).expect("request");

    let response = guest::router(provider()).oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
