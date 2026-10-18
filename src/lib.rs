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

cfg_select! {
    target_arch = "wasm32" => {
        use omnia_sdk::api::http::serve;
        use omnia_sdk::{
            BlobStore, Broadcast, Config, DocumentStore, HttpRequest, Identity, Publish,
            StateStore, TableStore,
        };
        use omnia_wasi_messaging::types::{Error, Message};
        use tracing::Level;
        use wasip3::exports::http::handler::Guest;
        use wasip3::http::types as p3;

        /// Bare provider backed by the default WASI capability implementations.
        ///
        /// Each capability trait ships its WASI-backed methods as defaults, so an
        /// empty `impl` per capability is the whole production wiring.
        pub struct Provider;

        impl BlobStore for Provider {}
        impl Broadcast for Provider {}
        impl Config for Provider {}
        impl DocumentStore for Provider {}
        impl HttpRequest for Provider {}
        impl Identity for Provider {}
        impl Publish for Provider {}
        impl StateStore for Provider {}
        impl TableStore for Provider {}

        /// WASI HTTP export.
        pub struct Http;
        wasip3::http::service::export!(Http);

        impl Guest for Http {
            #[omnia_wasi_otel::instrument(name = "http_guest_handle", level = Level::INFO)]
            async fn handle(request: p3::Request) -> Result<p3::Response, p3::ErrorCode> {
                serve(router(Provider), request).await
            }
        }

        /// WASI Messaging export.
        pub struct Messaging;
        omnia_wasi_messaging::export!(Messaging with_types_in omnia_wasi_messaging);

        impl omnia_wasi_messaging::incoming_handler::Guest for Messaging {
            #[omnia_wasi_otel::instrument(name = "messaging_guest_handle")]
            async fn handle(message: Message) -> Result<(), Error> {
                let router = messaging_router(Provider).await;
                omnia_sdk::api::messaging::handle(&router, message).await
            }
        }
    }
    _ => {}
}
