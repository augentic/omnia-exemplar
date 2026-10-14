//! The topic list: every stem's topics on one messaging router, one statement per topic.

use common::config;
use omnia_sdk::api::{Client, messaging};

use crate::{Capabilities, OWNER};

/// Build the exact-topic messaging router over one provider-owning [`Client`].
///
/// Each topic is registered under its `{env}-` qualified name, so a delivery
/// from another environment is unhandled rather than consumed. One statement
/// per topic between `Router::new(..)` and the trace, each added at the end
/// of the list: `let router = router.route(config::topic_for(&env, <suffix>),
/// messaging::consume(<handler>));` over a constant of `common::routes::topic`,
/// spelled in full.
pub async fn messaging_router<P: Capabilities>(provider: P) -> messaging::Router<P> {
    let env = config::env(&provider).await;
    let router = messaging::Router::new(Client::new(OWNER, provider));
    tracing::debug!(env, "messaging topics qualified");
    router
}
