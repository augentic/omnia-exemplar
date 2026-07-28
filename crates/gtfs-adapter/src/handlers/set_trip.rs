//! God-mode trip override.

use anyhow::Context as _;
use omnia_guest::api::{CallContext, Operation, Provider};
use omnia_guest::{Config, Error, Result, StateStore, bad_request};
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
    /// Process identifier (always 0).
    pub process: u32,
}

impl<P> Operation<P> for SetTripRequest
where
    P: Provider + Config + StateStore,
{
    type Error = Error;
    type Input = Self;
    type Output = SetTripReply;

    async fn call(input: Self, context: CallContext<'_, P>) -> Result<SetTripReply> {
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
}
