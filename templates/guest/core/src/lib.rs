//! # guest
//!
//! An Omnia WASI guest: every stem's routes merged onto one HTTP router and
//! every stem's topics registered on one messaging router, over one
//! provider. Only the WASI exports are `wasm32`-gated: the routers are
//! generic over their provider, so `tests/routes.rs` and `tests/messaging.rs`
//! build the production routers natively under `omnia_test::guest::Provider`
//! doubles.

mod messaging;
mod routes;

pub use self::messaging::messaging_router;
pub use self::routes::router;

/// The tenant that owns this deployment.
pub const OWNER: &str = "guest";

omnia_sdk::http!(router(omnia_sdk::WasiProvider));
omnia_sdk::messaging!(messaging_router(omnia_sdk::WasiProvider));
