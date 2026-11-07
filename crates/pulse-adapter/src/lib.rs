//! # Pulse Transformer
//!
//! Transforms Pulse messages into Motion events.

pub mod handler;
pub mod motion;
pub mod pulse;
pub mod stops;

use omnia_sdk::Capabilities;
use omnia_sdk::api::Handlers;

/// The stem's handlers, merged onto the root routers.
pub fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::from([handler::handlers()])
}
