//! The route list: every stem's HTTP routes on one router, one statement per route.

use omnia_sdk::api::Client;

use crate::{Capabilities, OWNER};

/// Build the HTTP router over one provider-owning [`Client`].
///
/// Omnia creates one component instance per request, so the router and the
/// client are built inside each call. One statement per route between
/// `Router::new()` and `with_state`, each added at the end of the list:
/// `let router = router.route(<path>, <method>(<handler>));` over a constant
/// of `common::routes::http` and a route function of `omnia_sdk::api::http`,
/// both spelled in full.
pub fn router<P: Capabilities>(provider: P) -> axum::Router {
    let router = axum::Router::new();
    router.with_state(Client::new(OWNER, provider))
}
