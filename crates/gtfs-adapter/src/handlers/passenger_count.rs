//! # Passenger Count
//!
//! This module stores occupancy status for a given vehicle and trip.

use acme_common::routes;
use omnia_guest::api::{CallContext, Operation, Provider};
use omnia_guest::{Error, Result, StateStore};
use serde::{Deserialize, Serialize};

use crate::state_keys;

const OCCUPANCY_STATUS_TTL: u64 = 3 * 60 * 60; // 3 hours

impl<P> Operation<P> for PassengerCountMessage
where
    P: Provider + StateStore,
{
    type Error = Error;
    type Input = Self;
    type Output = ();

    #[tracing::instrument(
        name = "passenger_count_message",
        skip_all,
        fields(
            owner = context.owner,
            vehicle_id = input.vehicle.id,
            topic = routes::topic::PASSENGER_COUNT,
        ),
    )]
    async fn call(input: Self, context: CallContext<'_, P>) -> Result<()> {
        let provider = context.provider;

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
