//! Motion AVL message processing.

use chrono::{DateTime, Utc};
use common::topic;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::messaging::Message;
use omnia_sdk::{Config, Error, HttpClient, Identity, Publisher, Result, StateStore, bad_request};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::location::Location;
use crate::{location, serial_data};

pub(crate) fn handlers<P: crate::Requires>() -> Handlers<P> {
    Handlers::new().topic(topic::PULSE_TO_MOTION, motion)
}

/// Errors raised while validating an inbound Motion message.
#[derive(Error, Debug)]
pub enum MotionError {
    /// The message timestamp is invalid (too old or future-dated).
    #[error("{0}")]
    BadTime(String),
}

impl MotionError {
    fn code(&self) -> String {
        match self {
            Self::BadTime(_) => "bad_time".to_string(),
        }
    }
}

impl From<MotionError> for Error {
    fn from(err: MotionError) -> Self {
        Self::bad_request(err.code(), err.to_string())
    }
}

pub(crate) async fn motion<P>(context: Context<P>, input: MotionMessage) -> Result<()>
where
    P: Config + HttpClient + Identity + Publisher + StateStore,
{
    let provider = context.provider();
    let message = input;

    // serial data event
    if message.event_type == EventType::SerialData {
        #[cfg(feature = "god-mode")]
        let message = crate::god_mode::apply_overrides(provider, message).await?;
        serial_data::process(provider, &message).await?;
        return Ok(());
    }

    // must be a location event
    let Some(location) = location::process(provider, &message).await? else {
        return Ok(());
    };

    let (message, key, topic) = match location {
        Location::VehiclePosition(feed) => (Message::json(&feed)?, feed.id, topic::GTFS_VP),
        Location::DeadReckoning(dr) => (Message::json(&dr)?, dr.id, topic::DEAD_RECKONING),
    };

    // publish
    context.publisher().publish(topic, &message.with_key(key)).await?;

    Ok(())
}

/// AVL telemetry emitted by the Motion tracking units on board vehicles.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionMessage {
    /// The type of event carried by the message.
    #[serde(rename = "eventType")]
    pub event_type: EventType,
    /// Identifiers for the emitting tracking unit.
    pub remote_data: Option<RemoteData>,
    /// Message-level metadata (timestamp).
    pub message_data: MessageData,
    /// GPS location data for location events.
    #[serde(default)]
    pub location_data: LocationData,
    /// Supplementary event data.
    #[serde(default)]
    pub event_data: EventData,
    /// Serial data for sign-on events.
    pub serial_data: Option<SerialData>,
}

impl MotionMessage {
    pub(crate) fn timestamp(&self) -> Result<i64> {
        DateTime::parse_from_rfc3339(&self.message_data.timestamp)
            .map(|dt| dt.with_timezone(&Utc).timestamp())
            .map_err(|e| bad_request!("invalid timestamp: {}", e))
    }

    pub(crate) fn vehicle_id(&self) -> Option<&str> {
        self.remote_data
            .as_ref()
            .and_then(|rd| rd.external_id.as_deref().or(rd.remote_name.as_deref()))
    }
}

/// The type of event carried by a [`MotionMessage`].
///
/// The aliases cover the casings used by each upstream source: `SerialData` /
/// `Location` from the pulse-adapter and train AVL feeds, and the snake/upper
/// variants from older Motion firmware.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum EventType {
    /// A serial data (sign-on) event.
    #[serde(rename = "serialData", alias = "SERIAL_DATA", alias = "SerialData")]
    SerialData,

    /// A GPS location event.
    #[serde(rename = "location", alias = "LOCATION", alias = "Location")]
    Location,

    /// Any other event type.
    #[serde(other)]
    Unknown,
}

/// Identifiers for the tracking unit that emitted the message.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteData {
    /// The vehicle's external identifier.
    pub external_id: Option<String>,
    /// The tracking unit's name.
    pub remote_name: Option<String>,
}

/// Message-level metadata.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageData {
    /// RFC 3339 timestamp of the reading.
    pub timestamp: String,
}

/// GPS location data for location events.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LocationData {
    /// Latitude in decimal degrees.
    pub latitude: Option<f64>,
    /// Longitude in decimal degrees.
    pub longitude: Option<f64>,
    /// Heading in degrees from true north.
    pub heading: Option<f64>,
    /// Speed in km/h.
    pub speed: Option<f64>,
    /// Odometer reading in metres.
    pub odometer: Option<f64>,
    /// GPS accuracy estimate.
    #[serde(default)]
    pub gps_accuracy: f64,
}

/// Supplementary event data.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EventData {
    /// Odometer reading in metres.
    pub odometer: Option<f64>,
}

/// Serial data attached to sign-on events.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerialData {
    /// The decoded serial payload, when the unit could parse it.
    pub decoded_serial_data: Option<DecodedSerialData>,
}

/// The decoded serial payload of a sign-on event.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecodedSerialData {
    /// The trip number keyed in by the driver.
    #[serde(alias = "tripNumber")]
    pub trip_number: Option<String>,
    /// The trip the vehicle signed on to.
    #[serde(alias = "tripId")]
    pub trip_id: Option<String>,
    /// The line the vehicle signed on to.
    #[serde(alias = "lineId")]
    pub line_id: Option<String>,
}
