//! Station-to-stop mapping and stop location lookups.

use anyhow::Context as _;
use axum::body::Body;
use omnia_sdk::{Config, HttpRequest, Result, StateStore, bad_gateway, server_error};
use serde::{Deserialize, Serialize};

/// Base URL of the static GTFS API.
pub const STATIC_API_URL: &str = "STATIC_API_URL";

const STOPS_CACHE_KEY: &str = "pulse:stops";
const STOPS_TTL_SECS: u64 = 300;

/// Stop information from GTFS
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct StopInfo {
    /// The stop code.
    pub stop_code: String,
    /// The stop latitude.
    pub stop_lat: f64,
    /// The stop longitude.
    pub stop_lon: f64,
}

/// Look up stop coordinates for a Pulse station code, caching the stop table in [`StateStore`].
///
/// # Errors
///
/// Returns an error when the static GTFS API fails, returns a non-success
/// status, cannot be deserialized, or lacks the mapped stop code.
pub async fn stop_info<P>(provider: &P, station: u32, is_arrival: bool) -> Result<Option<StopInfo>>
where
    P: Config + HttpRequest + StateStore,
{
    if !ACTIVE_STATIONS.contains(&station) {
        return Ok(None);
    }

    // get station's stop code
    let Some(stop_code) = station_stop(station) else {
        return Ok(None);
    };

    let stops = cached_stops(provider).await?;

    let Some(mut stop_info) = stops.into_iter().find(|stop| stop.stop_code == stop_code) else {
        return Err(server_error!("stop info not found for stop code {stop_code}"));
    };

    if !is_arrival {
        stop_info = departure(&stop_info.stop_code).unwrap_or(stop_info);
    }

    Ok(Some(stop_info))
}

async fn cached_stops<P>(provider: &P) -> Result<Vec<StopInfo>>
where
    P: Config + HttpRequest + StateStore,
{
    if let Some(bytes) = StateStore::get(provider, STOPS_CACHE_KEY).await? {
        return serde_json::from_slice(&bytes)
            .map_err(|error| server_error!("cached stops: {error}"));
    }

    let static_api_url =
        Config::get(provider, STATIC_API_URL).await.context("getting `STATIC_API_URL`")?;
    let request = http::Request::builder()
        .uri(format!("{static_api_url}/gtfs/stops?fields=stop_code,stop_lon,stop_lat"))
        .body(Body::empty())
        .context("building stops request")?;
    let response = HttpRequest::fetch(provider, request).await.context("fetching stops")?;
    if !response.status().is_success() {
        return Err(bad_gateway!("Static GTFS API returned {}", response.status()));
    }

    let bytes = response.into_body();
    let stops: Vec<StopInfo> =
        serde_json::from_slice(&bytes).context("deserializing stops response")?;
    let cached =
        serde_json::to_vec(&stops).map_err(|error| server_error!("stops cache: {error}"))?;
    StateStore::set(provider, STOPS_CACHE_KEY, &cached, Some(STOPS_TTL_SECS)).await?;
    Ok(stops)
}

const ACTIVE_STATIONS: &[u32] = &[0, 19, 40];

/// Map a station to its GTFS stop code.
const fn station_stop(station: u32) -> Option<&'static str> {
    match station {
        0 => Some("133"),
        19 => Some("9218"),
        40 => Some("134"),
        _ => None,
    }
}

/// Correct stops that have separate departure and arrival locations.
fn departure(stop_code: &str) -> Option<StopInfo> {
    let (stop_lat, stop_lon) = match stop_code {
        "133" => (-36.84448, 174.76915),
        "134" => (-37.20299, 174.90990),
        "9218" => (-36.99412, 174.8770),
        _ => return None,
    };
    Some(StopInfo {
        stop_code: stop_code.to_string(),
        stop_lat,
        stop_lon,
    })
}
