//! Motion AVL message processing.

use acme_common::{config, routes};
use chrono::{DateTime, Utc};
use omnia_guest::api::Context;
use omnia_guest::{
    Config, HttpRequest, Identity, Message, Publish, Result, StateStore, bad_request,
};
use serde::{Deserialize, Serialize};

use crate::location::Location;
use crate::{location, serial_data};

/// Processes a Motion AVL message, publishing a GTFS-realtime vehicle
/// position or dead-reckoning event when applicable.
///
/// # Errors
///
/// Returns an error when the payload cannot be processed or a provider
/// request fails.
#[omnia_guest::handler]
#[tracing::instrument(skip_all)]
pub async fn motion_message<P>(input: MotionMessage, context: Context<'_, P>) -> Result<()>
where
    P: Config + HttpRequest + Identity + Publish + StateStore,
{
    let provider = context.provider;
    let message = input;

    // serial data event
    if message.event_type == EventType::SerialData {
        #[cfg(feature = "god-mode")]
        let message = crate::god_mode::apply_overrides(message, provider).await?;
        serial_data::process(&message, provider).await?;
        return Ok(());
    }

    // must be a location event
    let Some(location) = location::process(&message, provider).await? else {
        return Ok(());
    };

    let (payload, key, suffix) = match location {
        Location::VehiclePosition(feed) => {
            (serde_json::to_vec(&feed)?, feed.id, routes::topic::GTFS_VP)
        }
        Location::DeadReckoning(dr) => {
            (serde_json::to_vec(&dr)?, dr.id, routes::topic::DEAD_RECKONING)
        }
    };

    let topic = config::topic(provider, suffix).await;

    // publish
    let mut message = Message::new(&payload);
    message.headers.insert("key".to_string(), key.clone());
    Publish::send(provider, &topic, &message).await?;

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
