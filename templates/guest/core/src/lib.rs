//! # guest
//!
//! An Omnia WASI guest: every stem's handlers merged onto one HTTP router and
//! one messaging router, over one provider. Only the WASI exports are
//! `wasm32`-gated: the routers are generic over their provider, so
//! `tests/routes.rs` and `tests/messaging.rs` build the production routers
//! natively under `omnia_test::guest::Provider` doubles.

use common::{Capabilities, config};
use omnia_sdk::api::{Client, Handlers, Router, http, messaging};

/// Every stem's handlers, merged onto the guest's routers.
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

/// Build the HTTP router over one provider-owning [`Client`].
///
/// Omnia creates one component instance per request, so the router and the
/// client are built inside each call.
pub fn http_router<P: Capabilities>(provider: P) -> Router<P> {
    handlers().with_state(Client::new(provider))
}

/// Build the exact-topic messaging router over one provider-owning [`Client`].
///
/// Each stem names its topics by bare suffix, and [`Handlers::map_topics`]
/// qualifies them all for the deployment's environment, so a delivery from
/// another environment is unhandled rather than consumed.
pub async fn topic_router<P: Capabilities>(provider: P) -> Router<P> {
    let env = config::env(&provider).await;
    handlers()
        .map_topics(|topic| config::topic_for(&env, topic))
        .with_state(Client::new(provider))
}

http::export!(http_router);
messaging::export!(topic_router);
