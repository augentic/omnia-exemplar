//! Trip Management API client and GTFS-realtime types.

use anyhow::{Context, Result};
use axum::body::Bytes;
use chrono::{Duration, NaiveDate, TimeZone, Timelike};
use chrono_tz::Tz;
use http::header::CONTENT_TYPE;
use http::{Method, StatusCode};
use omnia_sdk::{Config, HttpClient, Settings};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::warn;

/// The settings the Trip Management client reads.
#[derive(Debug, Settings)]
pub struct TripSettings {
    // Base URL of the Trip Management API.
    trip_management_url: String,
}

/// The local timezone of the (fictional) Acme transit network.
///
/// All schedule arithmetic (service dates, seconds-since-midnight offsets)
/// is performed in this zone. Acme deliberately runs on UTC to keep the
/// exemplar's fixtures reproducible — a real operator would set its actual
/// IANA zone here (`Pacific/Auckland`) and must not copy UTC-as-local.
pub const TIMEZONE: Tz = chrono_tz::UTC;

pub(crate) async fn instance<P>(
    provider: &P, trip_id: &str, service_date: &str, start_time: &str,
) -> Result<Option<TripInstance>>
where
    P: Config + HttpClient,
{
    let trips = fetch(provider, trip_id, service_date).await?;
    let mut iter = trips.into_iter();

    if let Some(first) = iter.next() {
        if first.has_error() {
            return Ok(Some(first));
        }

        if first.start_time == start_time {
            return Ok(Some(first));
        }

        for trip in iter {
            if trip.start_time == start_time {
                return Ok(Some(trip));
            }
        }
    }

    Ok(None)
}

pub(crate) async fn nearest<P>(
    provider: &P, trip_id: &str, event_timestamp: i64,
) -> Result<Option<TripInstance>>
where
    P: Config + HttpClient,
{
    let tz = TIMEZONE;
    let Some(event_dt) = tz.timestamp_opt(event_timestamp, 0).single() else {
        return Ok(None);
    };

    let current_date = event_dt.format("%Y%m%d").to_string();
    let mut trips = fetch(provider, trip_id, &current_date).await?;

    if trips.first().is_some_and(TripInstance::has_error) {
        return Ok(trips.into_iter().next());
    }

    if event_dt.hour() < 4 {
        let previous_date = (event_dt - Duration::days(1)).format("%Y%m%d").to_string();
        let previous = fetch(provider, trip_id, &previous_date).await?;
        if previous.first().is_some_and(TripInstance::has_error) {
            return Ok(previous.into_iter().next());
        }
        trips.extend(previous);
    }

    if trips.is_empty() {
        return Ok(None);
    }

    trips.sort_by(|left, right| {
        let event_ts = event_dt.timestamp();
        let left_diff = difference(event_ts, left, tz);
        let right_diff = difference(event_ts, right, tz);
        left_diff.cmp(&right_diff)
    });

    Ok(trips.into_iter().next())
}

async fn fetch<P>(provider: &P, trip_id: &str, service_date: &str) -> Result<Vec<TripInstance>>
where
    P: HttpClient + Config,
{
    let TripSettings { trip_management_url } = TripSettings::load(provider).await?;
    let endpoint = format!("{}/tripinstances", trip_management_url.trim_end_matches('/'));

    let payload = serde_json::json!({
        "tripIds": [trip_id],
        "serviceDate": service_date,
    });
    let body_bytes = serde_json::to_vec(&payload).context("serializing trip management payload")?;

    let request = http::Request::builder()
        .method(Method::POST)
        .uri(&endpoint)
        .header(CONTENT_TYPE, "application/json")
        .body(Bytes::from(body_bytes))
        .context("building Trip Management request")?;

    let response = provider.fetch(request).await.context("requesting trip instances")?;
    let status = response.status();
    let body = response.into_body();

    if status == StatusCode::NOT_FOUND {
        return Ok(Vec::new());
    }

    if !status.is_success() {
        warn!(%status, trip_id, service_date, "Trip Management API request failed");
        return Ok(vec![error_trip(service_date)]);
    }

    decode(&body)
        .with_context(|| format!("deserializing trip instances for {trip_id} on {service_date}"))
}

fn error_trip(service_date: &str) -> TripInstance {
    TripInstance {
        service_date: service_date.to_string(),
        error: true,
        ..TripInstance::default()
    }
}

fn decode(payload: &[u8]) -> Result<Vec<TripInstance>> {
    if payload.is_empty() {
        return Ok(Vec::new());
    }

    let value: Value = serde_json::from_slice(payload).context("parsing trip payload")?;
    extract(value)
}

fn extract(value: Value) -> Result<Vec<TripInstance>> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Array(items) => {
            let mut trips = Vec::new();
            for item in items {
                if matches!(&item, Value::Null)
                    || matches!(&item, Value::Object(map) if map.is_empty())
                {
                    continue;
                }
                let trip: TripInstance = serde_json::from_value(item)?;
                trips.push(trip);
            }
            Ok(trips)
        }
        Value::Object(mut map) => {
            if let Some(data) = map.remove("tripInstances") {
                return extract(data);
            }

            if map.is_empty() {
                return Ok(Vec::new());
            }

            let trip: TripInstance = serde_json::from_value(Value::Object(map))?;
            Ok(vec![trip])
        }
        other => {
            let trip: TripInstance = serde_json::from_value(other)?;
            Ok(vec![trip])
        }
    }
}

fn difference(event_ts: i64, trip: &TripInstance, tz: Tz) -> i64 {
    let trip_ts = timestamp(trip, tz).unwrap_or(event_ts);
    (event_ts - trip_ts).abs()
}

fn timestamp(trip: &TripInstance, tz: Tz) -> Option<i64> {
    let date = NaiveDate::parse_from_str(&trip.service_date, "%Y%m%d").ok()?;
    let total_seconds = parse_time(&trip.start_time)?;
    let days = total_seconds.div_euclid(86_400);
    let remaining = total_seconds.rem_euclid(86_400);

    let hours = u32::try_from(remaining / 3_600).ok()?;
    let minutes = u32::try_from((remaining % 3_600) / 60).ok()?;
    let seconds = u32::try_from(remaining % 60).ok()?;

    let date = date + Duration::days(days);
    let local = date.and_hms_opt(hours, minutes, seconds)?;
    tz.from_local_datetime(&local).single().map(|dt| dt.timestamp())
}

fn parse_time(time: &str) -> Option<i64> {
    let mut parts = time.split(':');
    let hours: i64 = parts.next()?.parse().ok()?;
    let minutes: i64 = parts.next()?.parse().ok()?;
    let seconds: i64 = parts.next()?.parse().ok()?;
    Some(hours * 3_600 + minutes * 60 + seconds)
}

/// A single scheduled instance of a trip on a service date.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TripInstance {
    /// The trip identifier.
    pub trip_id: String,
    /// The route the trip belongs to.
    pub route_id: String,
    /// The service date of the trip instance.
    pub service_date: String,
    /// The scheduled start time.
    pub start_time: String,
    /// The scheduled end time.
    pub end_time: String,
    /// The direction of travel, if known.
    pub direction_id: Option<i32>,
    /// Whether the trip was added outside the schedule.
    pub is_added_trip: bool,
    /// Whether the upstream API reported an error for this instance.
    #[serde(default)]
    pub error: bool,
}

impl TripInstance {
    /// Whether the upstream API reported an error for this instance.
    #[must_use]
    pub const fn has_error(&self) -> bool {
        self.error
    }

    /// Clone this instance with a different trip and route identifier.
    #[must_use]
    pub fn remap(&self, trip_id: &str, route_id: &str) -> Self {
        let mut clone = self.clone();
        clone.trip_id = trip_id.to_string();
        clone.route_id = route_id.to_string();
        clone
    }
}

/// A GTFS-realtime trip descriptor.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TripDescriptor {
    /// The trip identifier.
    pub trip_id: String,
    /// The route the trip belongs to.
    pub route_id: String,
    /// The scheduled start time.
    pub start_time: Option<String>,
    /// The trip's service date.
    pub start_date: Option<String>,
    /// The direction of travel, if known.
    pub direction_id: Option<i32>,
    /// The GTFS-realtime schedule relationship.
    pub schedule_relationship: Option<String>,
}

impl TripDescriptor {
    /// Schedule relationship for trips added outside the schedule.
    pub const ADDED: &'static str = "ADDED";
    /// Schedule relationship for scheduled trips.
    pub const SCHEDULED: &'static str = "SCHEDULED";
}

impl From<&TripInstance> for TripDescriptor {
    fn from(inst: &TripInstance) -> Self {
        Self {
            trip_id: inst.trip_id.clone(),
            route_id: inst.route_id.clone(),
            start_time: Some(inst.start_time.clone()),
            start_date: Some(inst.service_date.clone()),
            direction_id: inst.direction_id,
            schedule_relationship: Some(if inst.is_added_trip {
                Self::ADDED.to_string()
            } else {
                Self::SCHEDULED.to_string()
            }),
        }
    }
}

/// A dead-reckoning position estimate for a vehicle without a GPS fix.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadReckoningMessage {
    /// A unique identifier for the message.
    pub id: String,
    /// The time the source telemetry was received.
    pub received_at: i64,
    /// The odometer-based position estimate.
    pub position: PositionDr,
    /// The trip the vehicle is on.
    pub trip: TripDescriptor,
    /// The vehicle the estimate applies to.
    pub vehicle: VehicleDr,
}

/// An odometer-based position estimate.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionDr {
    /// The odometer reading in metres.
    pub odometer: f64,
}

/// The vehicle a dead-reckoning estimate applies to.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VehicleDr {
    /// The vehicle's unique identifier.
    pub id: String,
}

/// A GTFS-realtime feed entity carrying a vehicle position.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FeedEntity {
    /// The entity identifier (the vehicle identifier).
    pub id: String,
    /// The vehicle position payload.
    pub vehicle: Option<VehiclePosition>,
}

/// A GTFS-realtime vehicle position.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VehiclePosition {
    /// The vehicle's GPS position.
    pub position: Option<Position>,
    /// The trip the vehicle is serving.
    pub trip: Option<TripDescriptor>,
    /// The vehicle's descriptor.
    pub vehicle: Option<VehicleDescriptor>,
    /// The vehicle's occupancy status.
    pub occupancy_status: Option<String>,
    /// The time the position was recorded.
    pub timestamp: i64,
}

/// A GTFS-realtime position.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    /// Latitude in decimal degrees.
    pub latitude: Option<f64>,
    /// Longitude in decimal degrees.
    pub longitude: Option<f64>,
    /// Bearing in degrees from true north.
    pub bearing: Option<f64>,
    /// Speed in metres per second.
    pub speed: Option<f64>,
    /// Odometer reading in metres.
    pub odometer: Option<f64>,
}

/// A GTFS-realtime vehicle descriptor.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VehicleDescriptor {
    /// The vehicle's unique identifier.
    pub id: String,
    /// The vehicle's fleet label.
    pub label: Option<String>,
    /// The vehicle's license plate.
    pub license_plate: Option<String>,
}
