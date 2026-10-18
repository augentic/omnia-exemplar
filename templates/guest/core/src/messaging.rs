//! The topic list: every stem's topics registered on one messaging router, one statement per stem.

use common::{Capabilities, config};
use omnia_sdk::api::{Client, messaging};

use crate::OWNER;

/// Build the exact-topic messaging router over one provider-owning [`Client`].
///
/// Each stem registers its topics under their `{env}-` qualified names, so a
/// delivery from another environment is unhandled rather than consumed. One
/// statement per stem between `Router::new(..)` and the trace, each added at
/// the end of the list: `let router = <stem>::topics(router, &env);`.
/// Registering panics where two stems consume one topic, so the rung under
/// `tests/` holds the list to one consumer apiece.
pub async fn messaging_router<P: Capabilities>(provider: P) -> messaging::Router<P> {
    let env = config::env(&provider).await;
    let router = messaging::Router::new(Client::new(OWNER, provider));
    tracing::debug!(env, "messaging topics qualified");
    router
}
