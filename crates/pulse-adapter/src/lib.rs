//! # Pulse Transformer
//!
//! Transforms Pulse messages into Motion events.

pub mod handler;
pub mod motion;
pub mod pulse;
pub mod stops;

use omnia_sdk::api::Handlers;
use omnia_sdk::{Config, HttpClient, Identity, Publisher, StateStore};

/// The capabilities the stem's handlers reach, implemented by every provider that has them.
pub trait Requires:
    Config + HttpClient + Identity + Publisher + StateStore + Send + Sync + 'static
{
}

impl<P> Requires for P where
    P: Config + HttpClient + Identity + Publisher + StateStore + Send + Sync + 'static
{
}

/// The stem's handlers, merged onto the root routers.
pub fn handlers<P: Requires>() -> Handlers<P> {
    Handlers::from([handler::handlers()])
}
