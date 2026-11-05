//! Vehicle information lookup.

use axum::Json;
use axum::extract::Path;
use axum::routing::get;
use common::Capabilities;
use omnia_sdk::api::{Client, Context};
use omnia_sdk::{Config, HttpRequest, Identity, Result, StateStore};
use serde::{Deserialize, Serialize};

use crate::fleet::{self, Vehicle};
use crate::state_keys;
use crate::trip::TripInstance;

const PROCESS_ID: u32 = 0;

pub(crate) const VEHICLE_INFO: &str = "/info/{vehicle_id}";

pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {
    axum::Router::new().route(VEHICLE_INFO, get(vehicle_info))
}

/// A vehicle's current trip and fleet information.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VehicleInfoReply {
    /// Always `0`. Retained solely so the reply shape matches the legacy
    /// system this exemplar was ported from — do not copy into new services.
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

#[tracing::instrument(skip_all)]
pub(crate) async fn vehicle_info<P>(
    context: Context<P>, Path(vehicle_id): Path<String>,
) -> Result<Json<VehicleInfoReply>>
where
    P: Config + HttpRequest + Identity + StateStore + 'static,
{
    let provider = context.provider();

    let trip_key = state_keys::trip(&vehicle_id);
    let trip_info = if let Some(bytes) = StateStore::get(provider, &trip_key).await? {
        Some(serde_json::from_slice::<TripInstance>(&bytes)?)
    } else {
        None
    };

    let sign_on_key = state_keys::sign_on(&vehicle_id);
    let sign_on_time = StateStore::get(provider, &sign_on_key)
        .await?
        .map(|bytes| String::from_utf8_lossy(&bytes).to_string());

    let fleet_info = fleet::vehicle(&vehicle_id, provider).await?;

    Ok(Json(VehicleInfoReply {
        pid: PROCESS_ID,
        vehicle_id,
        sign_on_time,
        trip_info,
        fleet_info,
    }))
}
