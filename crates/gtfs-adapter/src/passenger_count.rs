//! # Passenger Count
//!
//! This module stores occupancy status for a given vehicle and trip.

use common::{Capabilities, config, topic};
use omnia_sdk::api::Context;
use omnia_sdk::api::messaging::{self, consume};
use omnia_sdk::{Result, StateStore};
use serde::{Deserialize, Serialize};

use crate::state_keys;

const OCCUPANCY_STATUS_TTL: u64 = 3 * 60 * 60; // 3 hours

pub(crate) fn topics<P: Capabilities>(
    router: messaging::Router<P>, env: &str,
) -> messaging::Router<P> {
    router.route(config::topic_for(env, topic::PASSENGER_COUNT), consume(passenger_count))
}

#[tracing::instrument(skip_all)]
pub(crate) async fn passenger_count<P>(
    input: PassengerCountMessage, context: Context<P>,
) -> Result<()>
where
    P: StateStore,
{
    let provider = context.provider();

    // create state key
    let vehicle_id = &input.vehicle.id;
    let Trip {
        trip_id,
        start_date,
        start_time,
    } = &input.trip;
    let key = state_keys::occupancy_status(vehicle_id, trip_id, start_date, start_time);

    // save occupancy status to state if set, otherwise remove
    if let Some(occupancy_status) = input.occupancy_status {
        let bytes = serde_json::to_vec(&occupancy_status)?;
        StateStore::set(provider, &key, &bytes, Some(OCCUPANCY_STATUS_TTL)).await?;
    } else {
        StateStore::delete(provider, &key).await?;
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
