//! # Pulse HTTP Connector
//!
//! Processes Pulse SOAP requests and forwards to the `pulse-adapter` topic.

mod handler;

pub use handler::*;
use omnia_guest::{Error, ErrorKind};
use thiserror::Error;

/// Errors raised while validating an inbound Pulse SOAP request.
///
/// Distinct from `pulse_adapter::PulseMessageError`: this type covers the
/// SOAP envelope received over HTTP, while the adapter's covers the embedded
/// train update message consumed from messaging.
#[derive(Error, Debug)]
pub enum PulseRequestError {
    /// The XML is invalid.
    #[error("{0}")]
    InvalidXml(String),
}

impl PulseRequestError {
    fn code(&self) -> String {
        match self {
            Self::InvalidXml(_) => "invalid_message".to_string(),
        }
    }
}

impl From<PulseRequestError> for Error {
    fn from(err: PulseRequestError) -> Self {
        Self::new(ErrorKind::BadRequest, err.code(), err.to_string())
    }
}

impl From<quick_xml::DeError> for PulseRequestError {
    fn from(err: quick_xml::DeError) -> Self {
        Self::InvalidXml(err.to_string())
    }
}
