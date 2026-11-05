//! Pulse HTTP Connector
//!
//! Listen for incoming Pulse SOAP requests and forward to the pulse-adapter
//! topic for validation and transformation to Motion events.

use std::fmt::{self, Display};

use anyhow::Context as _;
use axum::body::Bytes;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use common::{Capabilities, config, topic};
use omnia_sdk::api::{Context, http};
use omnia_sdk::{Config, Error, Message, Publish};
use serde::{Deserialize, Serialize};

pub(crate) const PULSE_XML: &str = "/inbound/xml";

/// The SOAP fault envelope answering requests that cannot be parsed or
/// fail validation.
const BAD_REQUEST_FAULT: Fault = Fault {
    status_code: 400,
    response: FaultMessage {
        message: "Bad Request",
    },
};

/// The SOAP fault envelope answering internal failures.
const SERVER_FAULT: Fault = Fault {
    status_code: 500,
    response: FaultMessage {
        message: "Internal Server Error",
    },
};

pub(crate) fn routes<P: Capabilities>() -> http::Routes<P> {
    http::Routes::new().route(PULSE_XML, post(pulse))
}

// The handler takes the raw body and parses the SOAP envelope itself, so a
// malformed body is answered with the vendor's XML `<Fault>` rather than an
// extractor's rejection.
pub(crate) async fn pulse<P>(context: Context<P>, body: Bytes) -> Result<PulseReply, Fault>
where
    P: Config + Publish + 'static,
{
    let provider = context.provider();

    // Parse and verify the message. Rejections are SOAP <Fault> envelopes
    // because the Pulse vendor protocol requires an XML fault body. This is
    // a vendor-protocol accommodation, not a general error-handling pattern
    // — prefer plain structured errors (see the domain error enums) unless
    // a wire protocol dictates otherwise.
    let request = PulseRequest::from_xml(&body).map_err(|error| {
        tracing::debug!("rejecting unparseable Pulse envelope: {error:#}");
        BAD_REQUEST_FAULT
    })?;

    let message = &request.body.receive_message.axml_message;
    if message.is_empty() || !message.contains("<ActualizarDatosTren>") {
        return Err(BAD_REQUEST_FAULT);
    }

    // forward to pulse-adapter topic
    let topic = config::topic(provider, topic::PULSE).await;

    let msg = Message::new(message.as_bytes());
    Publish::send(provider, &topic, &msg).await.map_err(|error| {
        tracing::error!("failed to forward Pulse message: {error:#}");
        SERVER_FAULT
    })?;

    Ok(PulseReply("OK"))
}

impl PulseRequest {
    /// Deserialize a Pulse SOAP envelope from raw XML.
    ///
    /// # Errors
    ///
    /// Returns an error if the XML cannot be deserialized.
    pub fn from_xml(input: &[u8]) -> anyhow::Result<Self> {
        quick_xml::de::from_reader(input).context("deserializing PulseRequest")
    }
}

/// Pulse SOAP Envelope for incoming [`ReceiveMessage`] requests
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PulseRequest {
    /// SOAP Body
    pub body: Body,
}

/// Pulse SOAP Body for [`ReceiveMessage`] requests
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Body {
    /// The wrapped train position message.
    pub receive_message: ReceiveMessage,
}

/// Pulse SOAP wrapper for train position messages.
#[derive(Debug, Clone, Deserialize)]
pub struct ReceiveMessage {
    /// The embedded train position XML document.
    #[serde(rename = "AXMLMessage")]
    pub axml_message: String,
}

/// Pulse SOAP Response, answered as a `text/xml` body.
#[derive(Debug, Clone, Serialize)]
#[serde(rename = "Return")]
pub struct PulseReply(pub &'static str);

impl IntoResponse for PulseReply {
    fn into_response(self) -> Response {
        match self.to_xml() {
            Ok(xml) => ([(CONTENT_TYPE, "text/xml")], xml).into_response(),
            Err(error) => Error::from(error).into_response(),
        }
    }
}

impl PulseReply {
    /// Serialize the reply into an XML body.
    ///
    /// # Errors
    ///
    /// Returns an error if the XML serialization fails.
    pub fn to_xml(&self) -> anyhow::Result<Vec<u8>> {
        let xml = quick_xml::se::to_string(&self).context("serializing PulseReply")?;
        Ok(xml.into_bytes())
    }
}

/// Pulse SOAP fault envelope returned on error.
///
/// The handler's error type: its [`IntoResponse`] implementation decides the
/// wire shape of rejections, so faults reach the client as `text/xml` with
/// the HTTP status the envelope itself carries.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Fault {
    status_code: u16,
    response: FaultMessage,
}

impl Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let xml = quick_xml::se::to_string(&self).map_err(|_e| fmt::Error)?;
        write!(f, "{xml}")
    }
}

impl std::error::Error for Fault {}

// Serializing this static envelope never fails in practice; the fallback is
// the plain-text form.
impl IntoResponse for Fault {
    fn into_response(self) -> Response {
        let status =
            StatusCode::from_u16(self.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        match quick_xml::se::to_string(&self) {
            Ok(xml) => (status, [(CONTENT_TYPE, "text/xml")], xml).into_response(),
            Err(_) => (status, self.response.message).into_response(),
        }
    }
}

/// The message carried by a SOAP [`Fault`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct FaultMessage {
    /// The fault description.
    pub message: &'static str,
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    #[test]
    fn deserialize_soap() {
        #[expect(clippy::disallowed_macros, reason = "unit test loads captured SOAP XML from disk")]
        let xml = include_str!("../data/receive-message.xml");
        let envelope = PulseRequest::from_xml(xml.as_bytes()).expect("should deserialize");

        let receive_message = envelope.body.receive_message;
        let message = receive_message.axml_message;

        assert!(message.contains("<ActualizarDatosTren>"), "message: {message}");
    }

    #[test]
    fn serialize_ok() {
        let xml = PulseReply("OK").to_xml().expect("should serialize");
        let xml = String::from_utf8(xml).expect("should be UTF-8");
        assert_eq!(xml, "<Return>OK</Return>");
    }

    #[test]
    fn serialize_error() {
        let xml = SERVER_FAULT.to_string();
        assert_eq!(
            xml,
            "<Fault><StatusCode>500</StatusCode><Response><Message>Internal Server Error</Message></Response></Fault>"
        );
    }

    /// A fault rendered as an HTTP response: a `text/xml` body whose HTTP
    /// status matches the envelope's own status code.
    #[tokio::test]
    async fn fault_into_response() {
        let response = BAD_REQUEST_FAULT.into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.headers().get(CONTENT_TYPE),
            Some(&HeaderValue::from_static("text/xml"))
        );

        let body =
            axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("should read body");
        assert_eq!(
            body.as_ref(),
            b"<Fault><StatusCode>400</StatusCode><Response><Message>Bad Request</Message></Response></Fault>"
        );
    }
}
