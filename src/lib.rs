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

mod messaging;
mod routes;

pub use self::messaging::messaging_router;
pub use self::routes::router;

/// The tenant that owns this deployment.
pub const OWNER: &str = "acme";

omnia_sdk::http!(router(omnia_sdk::WasiProvider));
omnia_sdk::messaging!(messaging_router(omnia_sdk::WasiProvider));
