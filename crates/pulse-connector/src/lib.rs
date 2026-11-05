//! # Pulse HTTP Connector
//!
//! Processes Pulse SOAP requests and forwards to the `pulse-adapter` topic.
//! The route carries its own codec: the body passes through undecoded and
//! the reply is the vendor's XML, since a malformed envelope is answered
//! with the vendor's `<Fault>` rather than the framework's JSON `400`.

pub mod handler;

use common::Capabilities;
use omnia_sdk::api::Client;

/// The stem's routes, merged onto the root router.
pub fn router<P: Capabilities>() -> axum::Router<Client<P>> {
    common::routers([
        // one entry per part with routes, each added at the end
        handler::routes(),
    ])
}
