//! The topic list: every crate's topics registered on one messaging router, one statement per crate.

use acme_common::{Capabilities, config};
use omnia_sdk::api::{Client, messaging};

use crate::OWNER;

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
