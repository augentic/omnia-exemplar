//! The route list: every stem's routes merged onto one router, one statement per stem.

use common::Capabilities;
use omnia_sdk::api::Client;

use crate::OWNER;

/// Build the HTTP router over one provider-owning [`Client`].
///
/// Omnia creates one component instance per request, so the router and the
/// client are built inside each call. One statement per stem between
/// `Router::new()` and `with_state`, each added at the end of the list:
/// `let router = router.merge(<stem>::router());`. Merging panics where two
/// stems serve one path under one method, so the rung under `tests/` holds
/// the list to one route apiece.
pub fn router<P: Capabilities>(provider: P) -> axum::Router {
    let router = axum::Router::new();
    router.with_state(Client::new(OWNER, provider))
}
