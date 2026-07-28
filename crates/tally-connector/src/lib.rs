//! # Tally APC connector
//!
//! Receives Tally passenger count requests and forwards to the
//! `realtime-tally-apc.v2` topic.

mod handler;
mod types;

pub use handler::*;
pub use types::*;
