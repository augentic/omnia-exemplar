//! Tally message types.

use serde::{Deserialize, Deserializer, Serialize};

/// Raw Tally payload emitted by the APC hardware on board a train.
/// The payload mirrors the legacy adapter schema so that parity can be
/// maintained against the historic records.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TallyMessage {
    /// Message format version supplied by Tally firmware.
    pub tly_vers: String,
    /// Message type that drives downstream routing.
    pub tly_type: String,
    /// Whether the vehicle is currently moving.
    pub driving: bool,
    /// Whether the vehicle considers itself stopped at a platform.
    pub atstop: bool,
    /// Indicates if the APC device is operating in service mode.
    pub operational: bool,
    /// Distance travelled since the current trip began (in metres).
    pub distance_start: i64,
    /// Trigger source that caused the message to be emitted.
    pub trigger: String,
    /// Hardware metadata describing the emitting device.
    pub device: Option<Device>,
    /// Timestamp metadata in UTC plus timezone hint.
    pub clock: Clock,
    /// Passenger information system snapshot included with the message.
    pub pis: Pis,
    /// Door-level passenger counters captured for this interval.
    pub doors: Vec<Door>,
    /// Scheduled arrival timestamp if supplied by the APC device.
    #[serde(default)]
    pub arrival_utc: Option<String>,
    /// Scheduled departure timestamp if supplied by the APC device.
    #[serde(default)]
    pub departure_utc: Option<String>,
    /// Distance to the previous stop, when available.
    #[serde(default)]
    pub distance_laststop: Option<i64>,
    /// Vehicle speed reported by the hardware (km/h).
    #[serde(default, deserialize_with = "deserialize_speed")]
    pub speed: Option<u32>,
    /// Geo-spatial waypoint associated with the reading.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wpt: Option<Waypoint>,
}

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
fn deserialize_speed<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(opt.and_then(|v| match v {
        serde_json::Value::Number(num) => num.as_f64().map(|f| f as u32),
        _ => None,
    }))
}

/// Tally message augmented with enrichment gathered from operator systems
/// (vehicle stop, trip and timetable context).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnrichedEvent {
    /// The original Tally event.
    #[serde(flatten)]
    pub event: TallyMessage,

    /// Optional stop identifier when a nearby train platform could be resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_id: Option<String>,
    /// Optional trip identifier when block allocation succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trip_id: Option<String>,
    /// Service date that the resolved trip belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Scheduled start time for the resolved trip.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
}

/// Metadata describing the APC device that emitted the event.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Device {
    /// Operating company associated with the vehicle.
    pub operator: String,
    /// Site identifier reported by the hardware (used to derive vehicle label).
    pub site: String,
    /// Hardware model reference.
    pub model: String,
    /// Device serial number.
    pub serial: String,
}

/// Timestamp metadata accompanying a Tally message.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Clock {
    /// UTC timestamp of the reading.
    pub utc: String,
    /// Timezone hint supplied by the device.
    pub tz: String,
}

/// Passenger information system context bundled with the Tally payload.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Pis {
    /// Line identifier displayed to passengers.
    pub line: String,
    /// Stop or station currently displayed.
    pub stop: String,
}

/// Door-level passenger counter values contained in a Tally reading.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Door {
    /// Door name or label as reported by the hardware.
    pub name: String,
    /// Number of passengers boarding through the door within the interval.
    #[serde(rename = "in")]
    pub passengers_in: u32,
    /// Number of passengers alighting through the door within the interval.
    #[serde(rename = "out")]
    pub passengers_out: u32,
    /// Door status flag indicating open/closed state transitions.
    pub st: String,
    /// Automatic reset timer counter (as reported by the hardware).
    pub art: u32,
    /// Optional error code emitted by the door sensor.
    #[serde(default)]
    pub err: Option<String>,
}

/// Geo-spatial waypoint describing where the Tally measurement occurred.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Waypoint {
    /// Satellite lock quality reported by the GPS receiver.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sat: Option<String>,
    /// Latitude of the waypoint.
    pub lat: String,
    /// Longitude of the waypoint.
    pub lon: String,
    /// Instantaneous speed reported (km/h).
    #[serde(default, deserialize_with = "deserialize_speed")]
    pub speed: Option<u32>,
}
