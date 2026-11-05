//! # Root-package guest
//!
//! Wires the shared transit handlers to WASI HTTP and WASI Messaging by
//! composition: every crate exposes its routes as an `axum::Router` over a
//! provider-owning `Client` and its topics as a registration on the
//! exact-topic `messaging::Router`, and the root merges them, one statement
//! per crate, in [`router`] and [`messaging_router`].
//!
//! This root-package layout (`src/lib.rs`) is the compiling reference for
//! new Omnia services and the shape the guest template seeds. Only the WASI
//! exports are `wasm32`-gated: the routers are generic over the provider so
//! the native rungs (`tests/routes.rs`, `tests/messaging.rs`) build the
//! production routers under `omnia_test::guest::Provider` doubles.

use common::{Capabilities, config};
use omnia_sdk::api::{Client, messaging};

/// The tenant that owns this deployment.
pub const OWNER: &str = "acme";

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

/// Build the exact-topic messaging router over one provider-owning [`Client`].
///
/// Each crate registers its topics under their `{env}-` qualified names, so a
/// topic from another environment is rejected as unhandled instead of
/// silently consumed. Router failures, handler errors with their structured
/// codes among them, flow back as `error.other` with the full display
/// string, since the WIT contract carries a string alone. Resolving the
/// environment reads `Config`, hence the `async`. One statement per crate
/// between `Router::new(..)` and the trace: registering panics where two
/// crates consume one topic, so the rung under `tests/` holds the list to
/// one consumer apiece.
pub async fn messaging_router<P: Capabilities>(provider: P) -> messaging::Router<P> {
    let env = config::env(&provider).await;
    let router = messaging::Router::new(Client::new(OWNER, provider));
    let router = gtfs_adapter::topics(router, &env);
    let router = pulse_adapter::topics(router, &env);
    tracing::debug!(env, "messaging topics qualified");
    router
}

omnia_sdk::http!(router(omnia_sdk::WasiProvider));
omnia_sdk::messaging!(messaging_router(omnia_sdk::WasiProvider));
