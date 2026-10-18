//! # Pulse Transformer
//!
//! Transforms Pulse messages into Motion events.

mod handler;
mod motion;
mod pulse;
mod stops;

use acme_common::{Capabilities, config, topic};
use omnia_sdk::Error;
use omnia_sdk::api::DecodeError;
use omnia_sdk::api::messaging::{self, Delivery, consume_with};
use thiserror::Error;

pub use self::handler::*;
pub use self::motion::*;
pub use self::pulse::*;
pub use self::stops::StopInfo;

/// The crate's topics, registered on the guest's messaging router under `env`.
#[must_use]
pub fn topics<P: Capabilities>(router: messaging::Router<P>, env: &str) -> messaging::Router<P> {
    router.route(config::topic_for(env, topic::PULSE), consume_with(pulse, decode_pulse_xml))
}

fn decode_pulse_xml(delivery: &Delivery) -> Result<PulseMessage, DecodeError> {
    PulseMessage::from_xml(&delivery.payload).map_err(|error| DecodeError::new(error.to_string()))
}

/// Errors raised while validating an inbound Pulse train update message.
///
/// This type covers the train update consumed from messaging.
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
        Self::BadRequest {
            code: err.code(),
            description: err.to_string(),
        }
    }
}

impl From<quick_xml::DeError> for PulseMessageError {
    fn from(err: quick_xml::DeError) -> Self {
        Self::InvalidXml(err.to_string())
    }
}
