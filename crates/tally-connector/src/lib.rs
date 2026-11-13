//! # Tally APC connector
//!
//! Receives Tally passenger count requests and forwards to the
//! `realtime-tally-apc.v2` topic.

pub mod handler;
pub mod types;

use omnia_sdk::Publisher;
use omnia_sdk::api::Handlers;

/// The capabilities the stem's handlers reach, implemented by every provider that has them.
pub trait Requires: Publisher + Send + Sync + 'static {}

impl<P> Requires for P where P: Publisher + Send + Sync + 'static {}

/// The stem's handlers, merged onto the root routers.
pub fn handlers<P: Requires>() -> Handlers<P> {
    Handlers::from([handler::handlers()])
}
