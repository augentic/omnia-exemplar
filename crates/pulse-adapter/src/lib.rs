//! # Pulse Transformer
//!
//! Transforms Pulse messages into Motion events.

pub mod handler;
pub mod motion;
pub mod pulse;
pub mod stops;

use common::Capabilities;
use omnia_sdk::api::http::Routes;
use omnia_sdk::api::messaging::Topics;

/// The stem's routes, merged onto the root router.
pub fn routes<P: Capabilities>() -> Routes<P> {
    Routes::from([
        // one entry per part with routes, each added at the end
    ])
}

/// The stem's topics, merged onto the root messaging router.
pub fn topics<P: Capabilities>() -> Topics<P> {
    Topics::from([
        // one entry per part with topics, each added at the end
        handler::topics(),
    ])
}
