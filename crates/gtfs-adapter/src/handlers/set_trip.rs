//! God-mode trip override.

use anyhow::Context as _;
use omnia_guest::api::{CallContext, Provider};
use omnia_guest::{Config, Result, StateStore, bad_request};
use serde::{Deserialize, Serialize};

use crate::god_mode;

/// Request to force a vehicle onto a specific trip.
#[derive(Debug, Clone, Deserialize)]
pub struct SetTripRequest {
    /// The vehicle to override.
    pub vehicle_id: String,
    /// The trip to force the vehicle onto (or `empty` to clear).
    pub trip_id: String,
}

/// Reply confirming a god-mode trip override.
#[derive(Debug, Clone, Serialize)]
pub struct SetTripReply {
    /// Human-readable result.
    pub message: String,
    /// Always `0`. Retained solely so the reply shape matches the legacy
    /// system this exemplar was ported from — do not copy into new services.
    pub process: u32,
}

#[omnia_guest::operation]
#[tracing::instrument(skip_all)]
async fn set_trip_request<P>(
    input: SetTripRequest, context: CallContext<'_, P>,
) -> Result<SetTripReply>
where
    P: Provider + Config + StateStore,
{
    let provider = context.provider;

    if !god_mode::is_enabled(provider).await? {
        return Err(bad_request!("God mode not enabled"));
    }

    god_mode::set_vehicle_to_trip(provider, input.vehicle_id, input.trip_id)
        .await
        .context("setting vehicle to trip")?;
    Ok(SetTripReply {
        message: "Ok".to_string(),
        process: 0,
    })
}
