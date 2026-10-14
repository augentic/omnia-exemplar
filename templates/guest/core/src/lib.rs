//! # guest
//!
//! An Omnia WASI guest: every stem's HTTP routes on one router and every
//! stem's topics on one messaging router, over one provider. Only the WASI
//! exports are `wasm32`-gated: the routers are generic over their provider,
//! so `tests/routes.rs` and `tests/messaging.rs` drive the production
//! tables natively under `omnia_test::guest::Provider` doubles.

mod messaging;
mod routes;

use omnia_sdk::{
    BlobStore, Broadcast, Config, DocumentStore, HttpRequest, Identity, Publish, StateStore,
    TableStore,
};

pub use self::messaging::messaging_router;
pub use self::routes::router;

/// The tenant that owns this deployment.
pub const OWNER: &str = "guest";

/// Every capability a stem may bound its provider on.
///
/// Both routers take any provider under this bound, so a stem reaching a
/// capability no stem reached before changes neither router's signature.
pub trait Capabilities:
    BlobStore
    + Broadcast
    + Config
    + DocumentStore
    + HttpRequest
    + Identity
    + Publish
    + StateStore
    + TableStore
    + Send
    + Sync
    + 'static
{
}

impl<P> Capabilities for P where
    P: BlobStore
        + Broadcast
        + Config
        + DocumentStore
        + HttpRequest
        + Identity
        + Publish
        + StateStore
        + TableStore
        + Send
        + Sync
        + 'static
{
}

/// Bare provider backed by the default WASI capability implementations.
///
/// Each capability trait ships its WASI-backed methods as defaults, so an
/// empty `impl` per capability is the whole production wiring.
#[cfg(target_arch = "wasm32")]
pub struct Provider;

#[cfg(target_arch = "wasm32")]
impl BlobStore for Provider {}
#[cfg(target_arch = "wasm32")]
impl Broadcast for Provider {}
#[cfg(target_arch = "wasm32")]
impl Config for Provider {}
#[cfg(target_arch = "wasm32")]
impl DocumentStore for Provider {}
#[cfg(target_arch = "wasm32")]
impl HttpRequest for Provider {}
#[cfg(target_arch = "wasm32")]
impl Identity for Provider {}
#[cfg(target_arch = "wasm32")]
impl Publish for Provider {}
#[cfg(target_arch = "wasm32")]
impl StateStore for Provider {}
#[cfg(target_arch = "wasm32")]
impl TableStore for Provider {}

/// WASI HTTP export.
#[cfg(target_arch = "wasm32")]
pub struct Http;
#[cfg(target_arch = "wasm32")]
wasip3::http::service::export!(Http);

#[cfg(target_arch = "wasm32")]
impl wasip3::exports::http::handler::Guest for Http {
    async fn handle(
        request: wasip3::http::types::Request,
    ) -> Result<wasip3::http::types::Response, wasip3::http::types::ErrorCode> {
        omnia_sdk::api::http::serve(router(Provider), request).await
    }
}

/// WASI Messaging export.
#[cfg(target_arch = "wasm32")]
pub struct Messaging;
#[cfg(target_arch = "wasm32")]
omnia_wasi_messaging::export!(Messaging with_types_in omnia_wasi_messaging);

#[cfg(target_arch = "wasm32")]
impl omnia_wasi_messaging::incoming_handler::Guest for Messaging {
    async fn handle(
        message: omnia_wasi_messaging::types::Message,
    ) -> Result<(), omnia_wasi_messaging::types::Error> {
        let router = messaging_router(Provider).await;
        omnia_sdk::api::messaging::handle(&router, message).await
    }
}
