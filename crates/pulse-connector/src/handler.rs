//! Pulse HTTP Connector
//!
//! Listen for incoming Pulse SOAP requests and forward to the pulse-adapter
//! topic for validation and transformation to Motion events.

use std::fmt::{self, Display};

use acme_common::{config, routes};
use anyhow::Context as _;
use http::{HeaderValue, StatusCode};
use omnia_guest::api::Context;
use omnia_guest::{Config, HttpError, Message, Publish};
use serde::{Deserialize, Serialize};

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

#[omnia_guest::handler]
async fn pulse_request<P>(input: PulseXml, context: Context<'_, P>) -> Result<PulseReply, Fault>
where
    P: Config + Publish,
{
    let provider = context.provider;

    // Parse and verify the message. Rejections are SOAP <Fault> envelopes
    // because the Pulse vendor protocol requires an XML fault body. This is
    // a vendor-protocol accommodation, not a general error-handling pattern
    // — prefer plain structured errors (see the domain error enums) unless
    // a wire protocol dictates otherwise.
    let request = PulseRequest::from_xml(&input.0).map_err(|error| {
        tracing::debug!("rejecting unparseable Pulse envelope: {error:#}");
        BAD_REQUEST_FAULT
    })?;

    let message = &request.body.receive_message.axml_message;
    if message.is_empty() || !message.contains("<ActualizarDatosTren>") {
        return Err(BAD_REQUEST_FAULT);
    }

    // forward to pulse-adapter topic
    let topic = config::topic(provider, routes::topic::PULSE).await;

    let msg = Message::new(message.as_bytes());
    Publish::send(provider, &topic, &msg).await.map_err(|error| {
        tracing::error!("failed to forward Pulse message: {error:#}");
        SERVER_FAULT
    })?;

    Ok(PulseReply("OK"))
}

/// The raw XML body of an incoming Pulse request.
///
/// The HTTP route passes the body through undecoded: parsing happens inside
/// the handler so that a malformed envelope is answered with the vendor's
/// SOAP [`Fault`] rather than the framework's plain-text decode error.
#[derive(Debug, Clone)]
pub struct PulseXml(pub Vec<u8>);

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

/// Pulse SOAP Response
#[derive(Debug, Clone, Serialize)]
#[serde(rename = "Return")]
pub struct PulseReply(pub &'static str);

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
/// The handler's error type: its [`HttpError`] conversion below decides the
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

/// Encode the fault as the vendor's `text/xml` response body.
///
/// Handler errors bypass the route's success encoder, so this conversion
/// alone decides the wire shape: [`HttpError::with_body`] carries the
/// pre-rendered envelope and content type to the response unchanged.
impl From<Fault> for HttpError {
    fn from(fault: Fault) -> Self {
        let status =
            StatusCode::from_u16(fault.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        quick_xml::se::to_string(&fault).map_or_else(
            // Unreachable in practice for this static envelope; degrade to
            // the plain-text form.
            |_| Self::new(status, fault.response.message),
            |xml| Self::with_body(status, HeaderValue::from_static("text/xml"), xml.into_bytes()),
        )
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
    use axum::response::IntoResponse as _;
    use http::header::CONTENT_TYPE;

    use super::*;

    #[test]
    fn deserialize_soap() {
        let xml = include_str!("../data/receive-message.xml");
        let envelope = PulseRequest::from_xml(xml.as_bytes()).expect("should deserialize");

        let receive_message = envelope.body.receive_message;
        let message = receive_message.axml_message;

        assert!(!message.is_empty());
        assert!(message.contains("<ActualizarDatosTren>"));
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

    /// The fault reaches the wire as a `text/xml` body whose HTTP status
    /// matches the envelope's own status code.
    #[tokio::test]
    async fn fault_responds_as_xml() {
        let response = HttpError::from(BAD_REQUEST_FAULT).into_response();

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
