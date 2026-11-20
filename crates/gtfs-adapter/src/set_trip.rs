//! God-mode trip override.

use anyhow::Context as _;
use axum::Json;
use axum::extract::Path;
use axum::routing::post;
use omnia_sdk::{Handlers, Result, bad_request};
use serde::{Deserialize, Serialize};

use crate::god_mode;

pub(crate) const SET_TRIP: &str = "/god-mode/set-trip/{vehicle_id}/{trip_id}";

pub(crate) fn handlers() -> Handlers {
    Handlers::new().route(SET_TRIP, post(set_trip))
}

/// Request to force a vehicle onto a specific trip: the route's two path parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct SetTripRequest {
    /// The vehicle to override.
    pub vehicle_id: String,
    /// The trip to force the vehicle onto (or `empty` to clear).
    pub trip_id: String,
}

/// Reply confirming a god-mode trip override.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SetTripReply {
    /// Human-readable result.
    pub message: String,
    /// Always `0`. Retained solely so the reply shape matches the legacy
    /// system this exemplar was ported from — do not copy into new services.
    pub process: u32,
}

pub(crate) async fn set_trip(Path(input): Path<SetTripRequest>) -> Result<Json<SetTripReply>> {
    if !god_mode::is_enabled().await? {
        return Err(bad_request!("God mode not enabled"));
    }

    god_mode::set_vehicle_to_trip(input.vehicle_id, input.trip_id)
        .await
        .context("setting vehicle to trip")?;
    Ok(Json(SetTripReply {
        message: "Ok".to_string(),
        process: 0,
    }))
}
