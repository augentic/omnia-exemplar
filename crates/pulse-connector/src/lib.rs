//! # Pulse HTTP Connector
//!
//! Processes Pulse SOAP requests and forwards to the `pulse-adapter` topic.

mod handler;

pub use handler::*;
use omnia_guest::Error;
use thiserror::Error;

/// Errors raised while validating an inbound Pulse message.
#[derive(Error, Debug)]
pub enum PulseError {
    /// The XML is invalid.
    #[error("{0}")]
    InvalidXml(String),
}

impl PulseError {
    fn code(&self) -> String {
        match self {
            Self::InvalidXml(_) => "invalid_message".to_string(),
        }
    }
}

impl From<PulseError> for Error {
    fn from(err: PulseError) -> Self {
        Self::BadRequest {
            code: err.code(),
            description: err.to_string(),
        }
    }
}

impl From<quick_xml::DeError> for PulseError {
    fn from(err: quick_xml::DeError) -> Self {
        Self::InvalidXml(err.to_string())
    }
}
