//! Station-to-stop mapping and stop location lookups.

use anyhow::{Context, Result, anyhow};
use bytes::Bytes;
use http_body_util::Empty;
use omnia_guest::{Config, HttpRequest};
use serde::{Deserialize, Serialize};

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

pub async fn stop_info<P>(
    _owner: &str, provider: &P, station: u32, is_arrival: bool,
) -> Result<Option<StopInfo>>
where
    P: Config + HttpRequest,
{
    if !ACTIVE_STATIONS.contains(&station) {
        return Ok(None);
    }

    // get station's stop code
    let Some(stop_code) = station_stop(station) else {
        return Ok(None);
    };

    let static_api_url = Config::get(provider, acme_common::config::STATIC_API_URL)
        .await
        .context("getting `STATIC_API_URL`")?;
    let request = http::Request::builder()
        .uri(format!("{static_api_url}/gtfs/stops?fields=stop_code,stop_lon,stop_lat"))
        .body(Empty::<Bytes>::new())
        .context("building stops request")?;
    let response = HttpRequest::fetch(provider, request).await.context("fetching stops")?;

    let bytes = response.into_body();
    let stops: Vec<StopInfo> =
        serde_json::from_slice(&bytes).context("deserializing stops response")?;

    let Some(mut stop_info) = stops.into_iter().find(|stop| stop.stop_code == stop_code) else {
        return Err(anyhow!("stop info not found for stop code {stop_code}"));
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
