//! # Pulse Transformer
//!
//! Transforms Pulse messages into Motion events.

mod handler;
mod motion;
mod pulse;
mod stops;

use omnia_guest::{Error, ErrorKind};
use thiserror::Error;

pub use self::handler::*;
pub use self::motion::*;
pub use self::pulse::*;
pub use self::stops::StopInfo;

/// Errors raised while validating an inbound Pulse train update message.
///
/// Distinct from `pulse_connector::PulseRequestError`: this type covers the
/// train update consumed from messaging, while the connector's covers the
/// SOAP envelope received over HTTP.
#[derive(Error, Debug)]
pub enum PulseMessageError {
    /// The message timestamp is invalid (too old or future-dated).
    #[error("{0}")]
    BadTime(String),

    /// The message contains no updates or the arrival/departure time is
    /// invalid (negative or 0).
    #[error("{0}")]
    NoUpdate(String),

    /// The XML is invalid.
    #[error("{0}")]
    InvalidXml(String),
}

impl PulseMessageError {
    fn code(&self) -> String {
        match self {
            Self::BadTime(_) => "bad_time".to_string(),
            Self::NoUpdate(_) => "no_update".to_string(),
            Self::InvalidXml(_) => "invalid_message".to_string(),
        }
    }
}

impl From<PulseMessageError> for Error {
    fn from(err: PulseMessageError) -> Self {
        Self::new(ErrorKind::BadRequest, err.code(), err.to_string())
    }
}

impl From<quick_xml::DeError> for PulseMessageError {
    fn from(err: quick_xml::DeError) -> Self {
        Self::InvalidXml(err.to_string())
    }
}
