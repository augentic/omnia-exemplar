//! # Pulse Transformer
//!
//! Transforms Pulse messages into Motion events.

pub mod handler;
pub mod motion;
pub mod pulse;
pub mod stops;

use common::Capabilities;
use omnia_sdk::api::messaging;

/// The stem's topics, registered on the root messaging router under `env`.
#[must_use]
pub fn topics<P: Capabilities>(router: messaging::Router<P>, env: &str) -> messaging::Router<P> {
    common::topics(
        router,
        env,
        [
            // one entry per part with topics, each added at the end
            handler::topics,
        ],
    )
}
