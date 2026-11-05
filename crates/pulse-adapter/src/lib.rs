//! # Pulse Transformer
//!
//! Transforms Pulse messages into Motion events.

pub mod handler;
pub mod motion;
pub mod pulse;
pub mod stops;

use common::Capabilities;
use omnia_sdk::api::messaging;

/// The stem's topics, merged onto the root messaging router.
pub fn topics<P: Capabilities>() -> messaging::Routes<P> {
    common::topics([
        // one entry per part with topics, each added at the end
        handler::topics(),
    ])
}
