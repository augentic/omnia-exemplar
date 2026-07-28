//! Motion AVL message processing.

use chrono::{DateTime, Utc};
use omnia_guest::api::{CallContext, Operation, Provider};
use omnia_guest::{
    Config, HttpRequest, Identity, Message, Publish, Result, StateStore, bad_request,
};
use serde::{Deserialize, Serialize};

use crate::location::Location;
use crate::{god_mode, location, serial_data};

impl<P> Operation<P> for MotionMessage
where
    P: Provider + Config + HttpRequest + Identity + Publish + StateStore,
{
    type Error = omnia_guest::Error;
    type Input = Self;
    type Output = ();

    async fn call(input: Self, context: CallContext<'_, P>) -> Result<()> {
        process(input, context.provider).await
    }
}

/// Processes a Motion AVL message, publishing a GTFS-realtime vehicle
/// position or dead-reckoning event when applicable.
///
/// # Errors
///
/// Returns an error when the payload cannot be processed or a provider
/// request fails.
pub async fn process<P>(message: MotionMessage, provider: &P) -> Result<()>
where
    P: Config + HttpRequest + Identity + Publish + StateStore,
{
    // serial data event
    if message.event_type == EventType::SerialData {
        let mut message = message.clone();
        if god_mode::is_enabled(provider).await? {
            god_mode::preprocess(provider, &mut message).await?;
        }
        serial_data::process(&message, provider).await?;
        return Ok(());
    }

    // must be a location event
    let Some(location) = location::process(&message, provider).await? else {
        return Ok(());
    };

    let (payload, key, topic) = match location {
        Location::VehiclePosition(feed) => {
            (serde_json::to_vec(&feed)?, feed.id, "realtime-gtfs-vp.v1")
        }
        Location::DeadReckoning(dr) => {
            (serde_json::to_vec(&dr)?, dr.id, "realtime-dead-reckoning.v1")
        }
    };

    let env = Config::get(provider, "ENV").await.unwrap_or_else(|_| "dev".to_string());
    let topic = format!("{env}-{topic}");

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
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum EventType {
    /// A serial data (sign-on) event.
    #[serde(rename = "serialData", alias = "SERIAL_DATA")]
    SerialData,

    /// A GPS location event.
    #[serde(rename = "location", alias = "LOCATION")]
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
