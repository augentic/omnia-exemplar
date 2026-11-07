//! # guest
//!
//! An Omnia WASI guest: every stem's handlers merged onto one router over one
//! provider. Only the WASI exports are `wasm32`-gated: the router is generic
//! over its provider, so `tests/routes.rs` and `tests/messaging.rs` build the
//! production router natively under `omnia_test::guest::Provider` doubles.

use omnia_sdk::Capabilities;
use omnia_sdk::api::{Client, Handlers, Router, http, messaging};

/// Build the guest router over one provider-owning [`Client`].
///
/// Omnia creates one component instance per request or delivery, so the
/// router and the client are built inside each call. The list holds one
/// `<stem>::handlers(),` per stem, each added at the end, in the array
/// form a stem lists its parts in; the comment heading the array keeps it
/// one entry per line, so two stems added at once merge as two lines.
/// Merging panics where two stems serve one path under one method or
/// consume one topic, so the rungs under `tests/` hold the list to one
/// handler apiece.
pub fn router<P: Capabilities>(provider: P) -> Router<P> {
    Handlers::from([
        // one `<stem>::handlers(),` per stem, each added at the end
    ])
    .with_state(Client::new(provider))
}

http::export!(router);
messaging::export!(router);
