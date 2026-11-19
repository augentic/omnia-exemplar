//! # Passenger Count
//!
//! This module stores occupancy status for a given vehicle and trip.

use std::time::Duration;

use common::topic;
use omnia_sdk::Result;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::keyvalue::{self, store};
use serde::{Deserialize, Serialize};

use crate::state_keys;

const OCCUPANCY_STATUS_TTL: Duration = Duration::from_hours(3);

pub(crate) fn handlers() -> Handlers {
    Handlers::new().topic(topic::PASSENGER_COUNT, passenger_count)
}

pub(crate) async fn passenger_count(_: Context, input: PassengerCountMessage) -> Result<()> {
    // create state key
    let vehicle_id = &input.vehicle.id;
    let Trip {
        trip_id,
        start_date,
        start_time,
    } = &input.trip;
    let key = state_keys::occupancy_status(vehicle_id, trip_id, start_date, start_time);

    // save occupancy status to state if set, otherwise remove
    let state = store::open(keyvalue::DEFAULT.to_owned()).await?;
    if let Some(occupancy_status) = input.occupancy_status {
        state.set_json(&key, &occupancy_status, Some(OCCUPANCY_STATUS_TTL)).await?;
    } else {
        state.delete(key, None).await??;
    }

    Ok(())
}

/// An occupancy status update for a vehicle on a trip.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PassengerCountMessage {
    /// The GTFS-realtime occupancy status, or `None` to clear it.
    pub occupancy_status: Option<String>,
    /// The vehicle the status applies to.
    pub vehicle: Vehicle,
    /// The trip the status applies to.
    pub trip: Trip,
    /// The time the status was recorded.
    pub timestamp: i64,
}

/// The vehicle an occupancy status applies to.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vehicle {
    /// The vehicle's unique identifier.
    pub id: String,
}

/// The trip an occupancy status applies to.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trip {
    /// The trip identifier.
    pub trip_id: String,
    /// The trip's service date.
    pub start_date: String,
    /// The trip's scheduled start time.
    pub start_time: String,
}
