//! God-mode trip overrides for testing and operations.
//!
//! This is an operational tool, not part of the production pipeline: it lets
//! an operator force a vehicle onto a specific trip while diagnosing
//! allocation issues. The whole module — and the HTTP handler that drives
//! it ([`crate::set_trip::SetTripRequest`]) — is compiled only when the
//! `god-mode` cargo feature is enabled, and does nothing at runtime unless
//! the `GOD_MODE_ENABLED` configuration key is set to a truthy value.
//!
//! Overrides live in the state store under the `god_mode:` prefix, separate
//! from the `motionGtfs:` pipeline state (see `state_keys`).

use std::collections::HashMap;

use anyhow::Result;
use omnia_sdk::{Config, StateStore, StateStoreExt as _};
use serde::{Deserialize, Serialize};

use crate::motion::{EventType, MotionMessage};

#[derive(Debug, Default, Serialize, Deserialize)]
struct GodModeState {
    overrides: HashMap<String, String>,
}

pub(crate) async fn set_vehicle_to_trip(
    state_store: &impl StateStore, vehicle_id: impl Into<String>, trip_id: impl Into<String>,
) -> Result<()> {
    let mut state = load_state(state_store).await?;
    state.overrides.insert(vehicle_id.into(), trip_id.into());
    save_state(state_store, &state).await
}

const KEY_GOD_MODE: &str = "god_mode:overrides";
const TTL_GOD_MODE: u64 = 24 * 60 * 60; // 24 hours

async fn load_state(state_store: &impl StateStore) -> Result<GodModeState> {
    Ok(state_store.get_json(KEY_GOD_MODE).await?.unwrap_or_default())
}

async fn save_state(state_store: &impl StateStore, state: &GodModeState) -> Result<()> {
    state_store.set_json(KEY_GOD_MODE, state, Some(TTL_GOD_MODE)).await
}

pub(crate) async fn apply_overrides<P>(provider: &P, event: MotionMessage) -> Result<MotionMessage>
where
    P: Config + StateStore,
{
    let mut event = event;
    if is_enabled(provider).await? {
        preprocess(provider, &mut event).await?;
    }
    Ok(event)
}

async fn preprocess(state_store: &impl StateStore, event: &mut MotionMessage) -> Result<()> {
    if event.event_type != EventType::SerialData {
        return Ok(());
    }

    let Some(remote_data) = event.remote_data.as_ref() else {
        return Ok(());
    };

    let Some(vehicle_id) = remote_data.external_id.as_deref() else {
        return Ok(());
    };

    let Some(serial) = event.serial_data.as_mut() else {
        return Ok(());
    };

    let Some(decoded) = serial.decoded_serial_data.as_mut() else {
        return Ok(());
    };

    let state = load_state(state_store).await?;
    if let Some(override_trip) = state.overrides.get(vehicle_id) {
        decoded.line_id = None;

        if override_trip == "empty" {
            decoded.trip_id = None;
            decoded.trip_number = None;
        } else {
            decoded.trip_id = Some(override_trip.clone());
            decoded.trip_number = Some(override_trip.clone());
        }
    }

    Ok(())
}

// Enables the god-mode trip override handler. Off unless set to a truthy
// value (`1`, `true`, `yes`, `on`).
pub(crate) const GOD_MODE_ENABLED: &str = "GOD_MODE_ENABLED";

pub(crate) async fn is_enabled(provider: &impl Config) -> Result<bool> {
    Ok(Config::get(provider, GOD_MODE_ENABLED).await.is_ok_and(|value| {
        let normalized = value.trim().to_ascii_lowercase();
        matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
    }))
}
