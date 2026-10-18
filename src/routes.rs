//! The route list: every crate's routes merged onto one router, one statement per crate.

use acme_common::Capabilities;
use omnia_sdk::api::Client;

use crate::OWNER;

/// Build the HTTP router over one provider-owning [`Client`].
///
/// Omnia creates one WASI component instance per HTTP request, so the router
/// and client are constructed inside each `handle` call; axum's route-state
/// clones share the client's provider allocation for that request only. One
/// statement per crate between `Router::new()` and `with_state`: merging
/// panics where two crates serve one path under one method, so the rung
/// under `tests/` holds the list to one route apiece.
pub fn router<P: Capabilities>(provider: P) -> axum::Router {
    let router = axum::Router::new();
    let router = router.merge(capability::router());
    let router = router.merge(docstore::router());
    let router = router.merge(gtfs_adapter::router());
    let router = router.merge(pattern::router());
    let router = router.merge(pulse_connector::router());
    let router = router.merge(sql::router());
    let router = router.merge(tally_connector::router());
    router.with_state(Client::new(OWNER, provider))
}
