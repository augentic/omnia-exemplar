//! Serial data (sign-on) event processing.

use std::time::Duration;

use jiff::Timestamp;
use omnia_sdk::keyvalue::{self, Bucket};
use omnia_sdk::{Result, bad_request};

use crate::motion::{DecodedSerialData, MotionError, MotionMessage};
use crate::state_keys;
use crate::trip::{self, TripInstance};

const TTL_TRIP_SERIAL: Duration = Duration::from_hours(4);
const TTL_SIGN_ON: Duration = Duration::from_hours(24);
const TTL_SERIAL_TIMESTAMP: Duration = Duration::from_hours(24);

const SERIAL_DATA_THRESHOLD: i64 = 900;

pub async fn process(message: &MotionMessage) -> Result<()> {
    let Some(vehicle_id) = message.vehicle_id() else {
        return Err(bad_request!("missing vehicle identifier"));
    };

    // validate timestamp
    let timestamp = message.timestamp()?;

    // is this a future-dated (by 900 secs) timestamp?
    if timestamp > Timestamp::now().as_second() + SERIAL_DATA_THRESHOLD {
        return Err(MotionError::BadTime("future-dated serial data message".to_string()).into());
    }

    let state = keyvalue::default().await?;
    update_timestamp(&state, timestamp, vehicle_id).await?;

    let Some(serial_data) = message.serial_data.as_ref() else {
        return Err(bad_request!("missing serialData"));
    };
    let Some(decoded) = serial_data.decoded_serial_data.as_ref() else {
        return Err(bad_request!("missing decoded serial data"));
    };

    allocate(&state, vehicle_id, decoded, timestamp).await
}

// Updates the timestamp if it is newer than the previously stored timestamp.
async fn update_timestamp(state: &Bucket, timestamp: i64, vehicle_id: &str) -> Result<()> {
    let key = state_keys::serial_timestamp(vehicle_id);

    // check previous timestamp
    if let Some(prev) = state.get_json::<i64>(&key).await?
        && prev >= timestamp
    {
        return Err(MotionError::BadTime("outdated serial data message".to_string()).into());
    }

    // store new timestamp
    state.set_json(&key, &timestamp, Some(TTL_SERIAL_TIMESTAMP)).await?;

    Ok(())
}

async fn allocate(
    state: &Bucket, vehicle_id: &str, decoded: &DecodedSerialData, event_timestamp: i64,
) -> Result<()> {
    let trip_key = state_keys::trip(vehicle_id);
    let sign_on_key = state_keys::sign_on(vehicle_id);
    let serial_timestamp_key = state_keys::serial_timestamp(vehicle_id);

    let Some(trip_id) = decoded.trip_id.as_deref() else {
        tracing::debug!(vehicle_id, "no trip id found, clearing state");

        state.remove(&sign_on_key).await?;
        state.remove(&trip_key).await?;
        state.remove(&serial_timestamp_key).await?;

        return Ok(());
    };

    let Some(prev) = state.get_json::<TripInstance>(&trip_key).await? else {
        return Ok(());
    };
    if prev.trip_id == trip_id {
        return Ok(());
    }

    if let Some(trip) = trip::nearest(trip_id, event_timestamp).await?
        && !trip.has_error()
    {
        return save_trip(state, vehicle_id, event_timestamp, trip).await;
    }

    state.remove(&sign_on_key).await?;
    state.remove(&trip_key).await?;
    state.remove(&serial_timestamp_key).await?;

    Ok(())
}

async fn save_trip(
    state: &Bucket, vehicle_id: &str, event_timestamp: i64, trip: TripInstance,
) -> Result<()> {
    let trip_key = state_keys::trip(vehicle_id);
    let sign_on_key = state_keys::sign_on(vehicle_id);

    state.set_json(&trip_key, &trip, Some(TTL_TRIP_SERIAL)).await?;
    state.set_json(&sign_on_key, &event_timestamp, Some(TTL_SIGN_ON)).await?;

    Ok(())
}
