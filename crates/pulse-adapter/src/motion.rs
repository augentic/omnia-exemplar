//! Motion event types for handling Motion data.

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize, Serializer};

use crate::stops::StopInfo;

/// Motion event.
/// N.B. that `@JsonProperty` descriptors are used for deserialisation only,
/// while the property name will be used when the data is serialised before
/// being published.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionEvent {
    /// The time the event was received.
    #[serde(serialize_with = "with_nanos")]
    pub received_at: DateTime<Utc>,

    /// The type of the event.
    pub event_type: EventType,

    /// Event data containing specific details about the event.
    pub event_data: EventData,

    /// Message data for the event.
    pub message_data: MessageData,

    /// Remote data associated with the event.
    pub remote_data: RemoteData,

    /// Location data for the event.
    pub location_data: LocationData,

    /// The identifier of the company associated with the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company_id: Option<u64>,

    /// Serial data associated with the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial_data: Option<SerialData>,
}

fn with_nanos<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let trunc = dt.to_rfc3339_opts(SecondsFormat::Millis, true);
    serializer.serialize_str(&trunc)
}

/// Motion event type.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventType {
    /// Location event.
    #[default]
    Location,

    /// Serial data event.
    SerialData,
}

/// Message data for the event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageData {
    /// Message identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<u64>,

    /// Message timestamp.
    pub timestamp: DateTime<Utc>,
}

impl Default for MessageData {
    fn default() -> Self {
        Self {
            message_id: None,
            timestamp: Utc::now(),
        }
    }
}

/// Remote data associated with the event.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteData {
    /// Remote identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_id: Option<u64>,

    /// Remote name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_name: Option<String>,

    /// External identifier.
    pub external_id: String,
}

/// Event data with specific details about the event.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventData {
    /// Event code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_code: Option<u64>,

    /// Odometer reading at the time of the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub odometer: Option<u64>,

    /// Nearest address to the event location.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nearest_address: Option<String>,

    /// Additional information about the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_info: Option<String>,
}

/// Location data for the event.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationData {
    /// Latitude of the event location.
    pub latitude: f64,

    /// Longitude of the event location.
    pub longitude: f64,

    /// Speed of the event location.
    pub speed: i64,

    /// GPS accuracy of the event location.
    pub gps_accuracy: i64,

    /// Heading of the event location.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<f64>,

    /// Kilometric point of the event location, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kilometric_point: Option<f64>,
}

impl From<StopInfo> for LocationData {
    fn from(stop: StopInfo) -> Self {
        Self {
            latitude: stop.stop_lat,
            longitude: stop.stop_lon,
            ..Self::default()
        }
    }
}

impl From<&StopInfo> for LocationData {
    fn from(stop: &StopInfo) -> Self {
        stop.clone().into()
    }
}

/// Serial data associated with the event.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerialData {
    /// Source of the serial data.
    pub source: u64,

    /// Raw serial bytes.
    pub serial_bytes: String,

    /// Decoded serial data.
    pub decoded: Option<DecodedSerialData>,
}

/// Decoded serial data, supports base64 format encoded and plain strings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecodedSerialData {
    /// Line identifier.
    pub line_id: String,
    /// Trip number.
    pub trip_number: String,
    /// Trip start time.
    pub start_at: String,
    /// Number of passengers on board.
    pub passengers_number: u32,
    /// Driver identifier.
    pub driver_id: String,
    /// Whether the trip is active.
    pub trip_active: bool,
    /// Whether the trip has ended.
    pub trip_ended: bool,
    /// Whether the trip-ended flag was present.
    pub has_trip_ended_flag: bool,
    /// Number of tag-ons.
    pub tag_ons: u32,
    /// Number of tag-offs.
    pub tag_offs: u32,
    /// Number of cash fares.
    pub cash_fares: u32,
}
