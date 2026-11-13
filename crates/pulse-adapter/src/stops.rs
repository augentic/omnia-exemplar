//! Station-to-stop mapping and stop location lookups.

use std::time::Duration;

use axum::body::Bytes;
use omnia_sdk::{
    Config, HttpClient, HttpClientExt as _, Result, Settings, StateStore, StateStoreExt as _,
    server_error,
};
use serde::{Deserialize, Serialize};

/// The settings the stop lookup reads.
#[derive(Debug, Settings)]
pub struct StopsSettings {
    // Base URL of the static GTFS API.
    static_api_url: String,
}

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

pub(crate) async fn stop_info<P>(
    provider: &P, station: u32, is_arrival: bool,
) -> Result<Option<StopInfo>>
where
    P: Config + HttpClient + StateStore,
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

const STOPS_CACHE_KEY: &str = "pulse.stops";
const STOPS_TTL: Duration = Duration::from_mins(5);

async fn cached_stops<P>(provider: &P) -> Result<Vec<StopInfo>>
where
    P: Config + HttpClient + StateStore,
{
    if let Some(bytes) = StateStore::get(provider, STOPS_CACHE_KEY).await? {
        match serde_json::from_slice(&bytes) {
            Ok(stops) => return Ok(stops),
            Err(error) => {
                tracing::warn!(%error, "cached stops entry unreadable; refetching");
            }
        }
    }

    let StopsSettings { static_api_url } = StopsSettings::load(provider).await?;
    let request = http::Request::builder()
        .uri(format!("{static_api_url}/gtfs/stops?fields=stop_code,stop_lon,stop_lat"))
        .body(Bytes::new())
        .map_err(|error| server_error!("building stops request: {error}"))?;
    let stops: Vec<StopInfo> = provider.fetch_json(request).await?;
    provider.set_json(STOPS_CACHE_KEY, &stops, Some(STOPS_TTL)).await?;
    Ok(stops)
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
