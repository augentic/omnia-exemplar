//! Serial data (sign-on) event processing.

use chrono::Utc;
use omnia_sdk::{Config, HttpRequest, Result, StateStore, StateStoreExt as _, bad_request};

use crate::motion::{DecodedSerialData, MotionError, MotionMessage};
use crate::state_keys;
use crate::trip::{self, TripInstance};

const TTL_TRIP_SERIAL_SECS: u64 = 4 * 60 * 60;
const TTL_SIGN_ON_SECS: u64 = 24 * 60 * 60;
const TTL_SERIAL_TIMESTAMP_SECS: u64 = 24 * 60 * 60;

const SERIAL_DATA_THRESHOLD: i64 = 900;

pub async fn process<P>(provider: &P, message: &MotionMessage) -> Result<()>
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

    allocate(provider, vehicle_id, decoded, timestamp).await
}

// Updates the timestamp if it is newer than the previously stored timestamp.
async fn update_timestamp(store: &impl StateStore, timestamp: i64, vehicle_id: &str) -> Result<()> {
    let key = state_keys::serial_timestamp(vehicle_id);

    // check previous timestamp
    if let Some(prev) = store.get_json::<i64>(&key).await?
        && prev >= timestamp
    {
        return Err(MotionError::BadTime("outdated serial data message".to_string()).into());
    }

    // store new timestamp
    store.set_json(&key, &timestamp, Some(TTL_SERIAL_TIMESTAMP_SECS)).await?;

    Ok(())
}

async fn allocate<P>(
    provider: &P, vehicle_id: &str, decoded: &DecodedSerialData, event_timestamp: i64,
) -> Result<()>
where
    P: Config + HttpRequest + StateStore,
{
    let trip_key = state_keys::trip(vehicle_id);
    let sign_on_key = state_keys::sign_on(vehicle_id);
    let serial_timestamp_key = state_keys::serial_timestamp(vehicle_id);

    let Some(trip_id) = decoded.trip_id.as_deref() else {
        tracing::debug!(vehicle_id, "no trip id found, clearing state");

        StateStore::delete(provider, &sign_on_key).await?;
        StateStore::delete(provider, &trip_key).await?;
        StateStore::delete(provider, &serial_timestamp_key).await?;

        return Ok(());
    };

    let Some(prev) = provider.get_json::<TripInstance>(&trip_key).await? else {
        return Ok(());
    };
    if prev.trip_id == trip_id {
        return Ok(());
    }

    if let Some(trip) = trip::nearest(provider, trip_id, event_timestamp).await?
        && !trip.has_error()
    {
        return save_trip(provider, vehicle_id, event_timestamp, trip).await;
    }

    StateStore::delete(provider, &sign_on_key).await?;
    StateStore::delete(provider, &trip_key).await?;
    StateStore::delete(provider, &serial_timestamp_key).await?;

    Ok(())
}

async fn save_trip<P>(
    provider: &P, vehicle_id: &str, event_timestamp: i64, trip: TripInstance,
) -> Result<()>
where
    P: StateStore,
{
    let trip_key = state_keys::trip(vehicle_id);
    let sign_on_key = state_keys::sign_on(vehicle_id);

    provider.set_json(&trip_key, &trip, Some(TTL_TRIP_SERIAL_SECS)).await?;
    provider.set_json(&sign_on_key, &event_timestamp, Some(TTL_SIGN_ON_SECS)).await?;

    Ok(())
}
