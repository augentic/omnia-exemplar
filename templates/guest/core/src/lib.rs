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

cfg_select! {
    target_arch = "wasm32" => {
        use omnia_sdk::{
            BlobStore, Broadcast, Config, DocumentStore, HttpRequest, Identity, Publish,
            StateStore, TableStore,
        };

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

        impl wasip3::exports::http::handler::Guest for Http {
            async fn handle(
                request: wasip3::http::types::Request,
            ) -> Result<wasip3::http::types::Response, wasip3::http::types::ErrorCode> {
                omnia_sdk::api::http::serve(router(Provider), request).await
            }
        }

        /// WASI Messaging export.
        pub struct Messaging;
        omnia_wasi_messaging::export!(Messaging with_types_in omnia_wasi_messaging);

        impl omnia_wasi_messaging::incoming_handler::Guest for Messaging {
            async fn handle(
                message: omnia_wasi_messaging::types::Message,
            ) -> Result<(), omnia_wasi_messaging::types::Error> {
                let router = messaging_router(Provider).await;
                omnia_sdk::api::messaging::handle(&router, message).await
            }
        }
    }
    _ => {}
}
