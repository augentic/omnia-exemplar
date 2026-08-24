//! Pulse HTTP Connector
//!
//! Listen for incoming Pulse SOAP requests and forward to the pulse-adapter
//! topic for validation and transformation to Motion events.

use std::fmt::{self, Display};

/// The default SOAP fault envelope returned on error.
const SOAP_FAULT: Fault = Fault {
    status_code: 500,
    response: FaultMessage {
        message: "Internal Server Error",
    },
};

use acme_common::{config, routes};
use anyhow::Context as _;
use omnia_guest::api::{CallContext, Provider};
use omnia_guest::{Config, Message, Publish, Result, bad_request};
use serde::{Deserialize, Serialize};

#[omnia_guest::operation]
async fn pulse_request<P>(input: PulseRequest, context: CallContext<'_, P>) -> Result<PulseReply>
where
    P: Provider + Config + Publish,
{
    let provider = context.provider;
    let message = &input.body.receive_message.axml_message;

    // Verify the message. The rejection body is a pre-rendered SOAP
    // <Fault> because the Pulse vendor protocol requires an XML fault
    // envelope. This is a vendor-protocol accommodation, not a general
    // error-handling pattern — prefer plain structured errors (see the
    // domain error enums) unless a wire protocol dictates otherwise.
    if message.is_empty() || !message.contains("<ActualizarDatosTren>") {
        return Err(bad_request!(SOAP_FAULT));
    }

    // forward to pulse-adapter topic
    let topic = config::topic(provider, routes::topic::PULSE).await;

    let msg = Message::new(message.as_bytes());
    Publish::send(provider, &topic, &msg).await?;

    Ok(PulseReply("OK"))
}

impl PulseRequest {
    /// Deserialize a Pulse SOAP envelope from raw XML.
    ///
    /// # Errors
    ///
    /// Returns an error if the XML cannot be deserialized.
    pub fn from_xml(input: &[u8]) -> Result<Self> {
        quick_xml::de::from_reader(input).context("deserializing PulseRequest").map_err(Into::into)
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

/// The message carried by a SOAP [`Fault`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct FaultMessage {
    /// The fault description.
    pub message: &'static str,
}

#[cfg(test)]
mod tests {
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
        let xml = SOAP_FAULT.to_string();
        assert_eq!(
            xml,
            "<Fault><StatusCode>500</StatusCode><Response><Message>Internal Server Error</Message></Response></Fault>"
        );
    }
}
