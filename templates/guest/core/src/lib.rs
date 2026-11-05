//! # guest
//!
//! An Omnia WASI guest: every stem's routes merged onto one HTTP router and
//! every stem's topics registered on one messaging router, over one
//! provider. Only the WASI exports are `wasm32`-gated: the routers are
//! generic over their provider, so `tests/routes.rs` and `tests/messaging.rs`
//! build the production routers natively under `omnia_test::guest::Provider`
//! doubles.

use common::{Capabilities, config};
use omnia_sdk::api::{Client, messaging};

use crate::OWNER;

/// The tenant that owns this deployment.
pub const OWNER: &str = "guest";

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

/// Build the exact-topic messaging router over one provider-owning [`Client`].
///
/// Each stem registers its topics under their `{env}-` qualified names, so a
/// delivery from another environment is unhandled rather than consumed. One
/// statement per stem between `Router::new(..)` and the trace, each added at
/// the end of the list: `let router = <stem>::topics(router, &env);`.
/// Registering panics where two stems consume one topic, so the rung under
/// `tests/` holds the list to one consumer apiece.
pub async fn messaging_router<P: Capabilities>(provider: P) -> messaging::Router<P> {
    let env = config::env(&provider).await;
    let router = messaging::Router::new(Client::new(OWNER, provider));
    tracing::debug!(env, "messaging topics qualified");
    router
}

omnia_sdk::http!(router(omnia_sdk::WasiProvider));
omnia_sdk::messaging!(messaging_router(omnia_sdk::WasiProvider));
