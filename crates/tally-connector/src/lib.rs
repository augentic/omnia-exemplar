//! # Tally APC connector
//!
//! Receives Tally passenger count requests and forwards to the
//! `realtime-tally-apc.v2` topic.

mod handler;
mod types;

use acme_common::Capabilities;
pub use handler::*;
use omnia_sdk::api::Client;
use omnia_sdk::api::http::post;
pub use types::*;

/// Tally passenger-count (APC) ingress.
pub const APC: &str = "/api/apc";

/// The crate's routes, merged onto the guest's router.
pub fn router<P: Capabilities>() -> axum::Router<Client<P>> {
    axum::Router::new().route(APC, post(tally))
}
