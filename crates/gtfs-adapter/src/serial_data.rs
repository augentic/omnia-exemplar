//! Serial data (sign-on) event processing.

use anyhow::Context as _;
use chrono::Utc;
use omnia_guest::{Config, HttpRequest, Result, StateStore, bad_request};

use crate::trip::{self, TripInstance};
use crate::{DecodedSerialData, MotionError, MotionMessage};

const TTL_TRIP_SERIAL_SECS: u64 = 4 * 60 * 60;
const TTL_SIGN_ON_SECS: u64 = 24 * 60 * 60;
const TTL_SERIAL_TIMESTAMP_SECS: u64 = 24 * 60 * 60;

const SERIAL_DATA_THRESHOLD: i64 = 900;

// Processes Motion serial data events, updating allocations and state.
pub async fn process<P>(message: &MotionMessage, provider: &P) -> Result<()>
where
    P: Config + HttpRequest + StateStore,
{
    let Some(vehicle_id) = message.vehicle_id() else {
        return Err(bad_request!("missing vehicle identifier"));
    };

    // validate timestamp
    let timestamp = message.timestamp()?;

    // is this a future-dated (by 900 secs) timestamp?
    if timestamp > Utc::now().timestamp() + SERIAL_DATA_THRESHOLD {
        return Err(MotionError::BadTime("future-dated serial data message".to_string()).into());
    }

    update_timestamp(provider, timestamp, vehicle_id).await?;

    let Some(serial_data) = message.serial_data.as_ref() else {
        return Err(bad_request!("missing serialData"));
    };
    let Some(decoded) = serial_data.decoded_serial_data.as_ref() else {
        return Err(bad_request!("missing decoded serial data"));
    };

    allocate(vehicle_id, decoded, timestamp, provider).await
}

// Updates the timestamp if it is newer than the previously stored timestamp.
async fn update_timestamp(store: &impl StateStore, timestamp: i64, vehicle_id: &str) -> Result<()> {
    let key = format!("motionGtfs:serialTimestamp:{vehicle_id}");

    // check previous timestamp
    let previous = StateStore::get(store, &key).await?;
    if serde_json::from_value::<i64>(previous.into()).is_ok_and(|prev| prev >= timestamp) {
        return Err(MotionError::BadTime("outdated serial data message".to_string()).into());
    }

    // store new timestamp
    let value = serde_json::to_vec(&timestamp).context("failed to serialize timestamp")?;
    StateStore::set(store, &key, &value, Some(TTL_SERIAL_TIMESTAMP_SECS)).await?;

    Ok(())
}

async fn allocate<P>(
    vehicle_id: &str, decoded: &DecodedSerialData, event_timestamp: i64, provider: &P,
) -> Result<()>
where
    P: Config + HttpRequest + StateStore,
{
    let trip_key = format!("motionGtfs:trip:vehicle:{vehicle_id}");
    let sign_on_key = format!("motionGtfs:vehicle:signOn:{vehicle_id}");
    let serial_timestamp_key = format!("motionGtfs:serialTimestamp:{vehicle_id}");

    let Some(trip_id) = decoded.trip_id.as_deref() else {
        tracing::debug!(vehicle_id, "no trip id found, clearing state");

        StateStore::delete(provider, &sign_on_key).await?;
        StateStore::delete(provider, &trip_key).await?;
        StateStore::delete(provider, &serial_timestamp_key).await?;

        return Ok(());
    };

    let Some(prev) = StateStore::get(provider, &trip_key).await? else {
        return Ok(());
    };
    if serde_json::from_slice::<TripInstance>(&prev).is_ok_and(|t| t.trip_id == trip_id) {
        return Ok(());
    }

    if let Some(trip) = trip::get_nearest(trip_id, event_timestamp, provider).await?
        && !trip.has_error()
    {
        return save_trip(vehicle_id, event_timestamp, trip, provider).await;
    }

    StateStore::delete(provider, &sign_on_key).await?;
    StateStore::delete(provider, &trip_key).await?;
    StateStore::delete(provider, &serial_timestamp_key).await?;

    Ok(())
}

async fn save_trip<P>(
    vehicle_id: &str, event_timestamp: i64, trip: TripInstance, provider: &P,
) -> Result<()>
where
    P: StateStore,
{
    let trip_key = format!("motionGtfs:trip:vehicle:{vehicle_id}");
    let sign_on_key = format!("motionGtfs:vehicle:signOn:{vehicle_id}");

    let trip_bytes = serde_json::to_vec(&trip).context("failed to serialize trip")?;
    StateStore::set(provider, &trip_key, &trip_bytes, Some(TTL_TRIP_SERIAL_SECS)).await?;

    let timestamp_bytes =
        serde_json::to_vec(&event_timestamp).context("failed to serialize message timestamp")?;
    StateStore::set(provider, &sign_on_key, &timestamp_bytes, Some(TTL_SIGN_ON_SECS)).await?;

    Ok(())
}
