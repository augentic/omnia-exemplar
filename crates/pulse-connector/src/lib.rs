//! # Pulse HTTP Connector
//!
//! Processes Pulse SOAP requests and forwards to the `pulse-adapter` topic.
//! The route carries its own codec: the body passes through undecoded and
//! the reply is the vendor's XML, since a malformed envelope is answered
//! with the vendor's `<Fault>` rather than the framework's JSON `400`.

pub mod handler;

use omnia_sdk::Publisher;
use omnia_sdk::api::Handlers;

/// The capabilities the stem's handlers reach, implemented by every provider that has them.
pub trait Requires: Publisher + Send + Sync + 'static {}

impl<P> Requires for P where P: Publisher + Send + Sync + 'static {}

/// The stem's handlers, merged onto the root routers.
pub fn handlers<P: Requires>() -> Handlers<P> {
    Handlers::from([handler::handlers()])
}
