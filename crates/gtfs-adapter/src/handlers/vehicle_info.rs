//! Vehicle information lookup.

use common::fleet::{self, Vehicle};
use omnia_guest::api::{CallContext, Operation, Provider};
use omnia_guest::{Config, Error, HttpRequest, Identity, Result, StateStore};
use serde::{Deserialize, Serialize};

use crate::trip::TripInstance;

const PROCESS_ID: u32 = 0;

/// Request for a vehicle's current trip and fleet information.
#[derive(Debug, Clone, Deserialize)]
pub struct VehicleInfoRequest {
    /// The vehicle to look up.
    pub vehicle_id: String,
}

/// A vehicle's current trip and fleet information.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VehicleInfoReply {
    /// Process identifier (always 0).
    pub pid: u32,
    /// The vehicle's unique identifier.
    pub vehicle_id: String,
    /// The time the vehicle signed on, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sign_on_time: Option<String>,
    /// The vehicle's current trip, if allocated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trip_info: Option<TripInstance>,
    /// The vehicle's fleet record, if found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fleet_info: Option<Vehicle>,
}

impl<P> Operation<P> for VehicleInfoRequest
where
    P: Provider + Config + HttpRequest + Identity + StateStore,
{
    type Error = Error;
    type Input = Self;
    type Output = VehicleInfoReply;

    async fn call(input: Self, context: CallContext<'_, P>) -> Result<VehicleInfoReply> {
        let provider = context.provider;
        let vehicle_id = input.vehicle_id;

        let trip_key = format!("motionGtfs:trip:vehicle:{vehicle_id}");
        let trip_info = if let Some(bytes) = StateStore::get(provider, &trip_key).await? {
            Some(serde_json::from_slice::<TripInstance>(&bytes)?)
        } else {
            None
        };

        let sign_on_key = format!("motionGtfs:vehicle:signOn:{vehicle_id}");
        let sign_on_time = StateStore::get(provider, &sign_on_key)
            .await?
            .map(|bytes| String::from_utf8_lossy(&bytes).to_string());

        let fleet_info = fleet::vehicle(&vehicle_id, provider).await?;

        Ok(VehicleInfoReply {
            pid: PROCESS_ID,
            vehicle_id,
            sign_on_time,
            trip_info,
            fleet_info,
        })
    }
}
