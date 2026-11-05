//! # Pulse HTTP Connector
//!
//! Processes Pulse SOAP requests and forwards to the `pulse-adapter` topic.
//! The route carries its own codec: the body passes through undecoded and
//! the reply is the vendor's XML, since a malformed envelope is answered
//! with the vendor's `<Fault>` rather than the framework's JSON `400`.

pub mod handler;

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
