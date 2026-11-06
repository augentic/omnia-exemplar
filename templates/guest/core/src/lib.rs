//! # guest
//!
//! An Omnia WASI guest: every stem's handlers merged onto one router over one
//! provider. Only the WASI exports are `wasm32`-gated: the router is generic
//! over its provider, so `tests/routes.rs` and `tests/messaging.rs` build the
//! production router natively under `omnia_test::guest::Provider` doubles.

use common::Capabilities;
use omnia_sdk::api::{Client, Handlers, Router, http, messaging};

/// Every stem's handlers, merged onto the guest's router.
///
/// The list holds one `<stem>::handlers(),` per stem, each added at the
/// end, in the array form a stem lists its parts in. Merging panics where
/// two stems serve one path under one method or consume one topic, so the
/// rungs under `tests/` hold the list to one handler apiece.
pub fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::from([
        // one handlers() per stem, each added at the end
    ])
}

/// Build the guest router over one provider-owning [`Client`].
///
/// Omnia creates one component instance per request or delivery, so the
/// router and the client are built inside each call.
pub fn router<P: Capabilities>(provider: P) -> Router<P> {
    handlers().with_state(Client::new(provider))
}

http::export!(router);
messaging::export!(router);
