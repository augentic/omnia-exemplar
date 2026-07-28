//! Station-to-stop mapping and stop location lookups.

use std::collections::HashMap;
use std::sync::LazyLock;

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
    let Some(stop_code) = STATION_STOP.get(&station) else {
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

    let Some(mut stop_info) = stops.into_iter().find(|stop| stop.stop_code == *stop_code) else {
        return Err(anyhow!("stop info not found for stop code {stop_code}"));
    };

    if !is_arrival {
        stop_info = DEPARTURES.get(&stop_info.stop_code).cloned().unwrap_or(stop_info);
    }

    Ok(Some(stop_info))
}

const ACTIVE_STATIONS: &[u32] = &[0, 19, 40];

static STATION_STOP: LazyLock<HashMap<u32, &str>> =
    LazyLock::new(|| HashMap::from([(0, "133"), (19, "9218"), (40, "134")]));

// Correct stops that have separate departure and arrival locations.
static DEPARTURES: LazyLock<HashMap<String, StopInfo>> = LazyLock::new(|| {
    HashMap::from([
        (
            "133".to_string(),
            StopInfo {
                stop_code: "133".to_string(),
                stop_lat: -36.84448,
                stop_lon: 174.76915,
            },
        ),
        (
            "134".to_string(),
            StopInfo {
                stop_code: "134".to_string(),
                stop_lat: -37.20299,
                stop_lon: 174.90990,
            },
        ),
        (
            "9218".to_string(),
            StopInfo {
                stop_code: "9218".to_string(),
                stop_lat: -36.99412,
                stop_lon: 174.8770,
            },
        ),
    ])
});
