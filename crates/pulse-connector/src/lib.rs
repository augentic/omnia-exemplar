//! # Pulse HTTP Connector
//!
//! Processes Pulse SOAP requests and forwards to the `pulse-adapter` topic.
//! The route carries its own codec: the body passes through undecoded and
//! the reply is the vendor's XML, since a malformed envelope is answered
//! with the vendor's `<Fault>` rather than the framework's JSON `400`.

mod handler;

use acme_common::Capabilities;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
pub use handler::*;
use omnia_sdk::HttpError;
use omnia_sdk::api::http::{MethodFilter, RawRequest, handle_with};
use omnia_sdk::api::{Client, DecodeError};

/// Pulse SOAP/XML position ingress.
pub const PULSE_XML: &str = "/inbound/xml";

/// The crate's routes, merged onto the guest's router.
pub fn router<P: Capabilities>() -> axum::Router<Client<P>> {
    axum::Router::new().route(
        PULSE_XML,
        handle_with(
            MethodFilter::POST,
            pulse,
            |raw: RawRequest<'_>| decode_pulse(raw.body),
            |reply| encode_pulse(&reply),
        ),
    )
}

// The handler parses the SOAP envelope itself so a malformed body is
// answered with the vendor's XML `<Fault>`; a decoder that failed here would
// reach the client as the framework's JSON `invalid_request` 400.
#[expect(clippy::unnecessary_wraps, reason = "the route codec requires a fallible decoder")]
fn decode_pulse(body: &[u8]) -> Result<PulseXml, DecodeError> {
    Ok(PulseXml(body.to_vec()))
}

fn encode_pulse(reply: &PulseReply) -> Response {
    match reply.to_xml() {
        Ok(xml) => ([(CONTENT_TYPE, "text/xml")], xml).into_response(),
        Err(error) => HttpError::from(error).into_response(),
    }
}
