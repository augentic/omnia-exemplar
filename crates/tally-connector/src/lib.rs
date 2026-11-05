//! # Tally APC connector
//!
//! Receives Tally passenger count requests and forwards to the
//! `realtime-tally-apc.v2` topic.

pub mod handler;
pub mod types;

use common::Capabilities;
use omnia_sdk::api::{http, messaging};

/// The stem's routes, merged onto the root router.
pub fn routes<P: Capabilities>() -> http::Routes<P> {
    http::Routes::from([
        // one entry per part with routes, each added at the end
        handler::routes(),
    ])
}

/// The stem's topics, merged onto the root messaging router.
pub fn topics<P: Capabilities>() -> messaging::Routes<P> {
    messaging::Routes::from([
        // one entry per part with topics, each added at the end
    ])
}
