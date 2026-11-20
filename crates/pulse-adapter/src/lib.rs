//! # Pulse Transformer
//!
//! Transforms Pulse messages into Motion events.

pub mod handler;
pub mod motion;
pub mod pulse;
pub mod stops;

use omnia_sdk::Handlers;

/// The stem's handlers, merged onto the root router.
pub fn handlers() -> Handlers {
    Handlers::from([handler::handlers()])
}
