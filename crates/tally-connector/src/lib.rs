//! # Tally APC connector
//!
//! Receives Tally passenger count requests and forwards to the
//! `realtime-tally-apc.v2` topic.

pub mod handler;
pub mod types;

use common::Capabilities;
use omnia_sdk::api::Handlers;

/// The stem's handlers, merged onto the root routers.
pub fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::from([handler::handlers()])
}
