//! Block Management API client.

use anyhow::{Context, Result};
use bytes::Bytes;
use http::Method;
use http::header::{AUTHORIZATION, CACHE_CONTROL, IF_NONE_MATCH};
use http_body_util::Empty;
use omnia_guest::{Config, HttpRequest, Identity};
use serde::{Deserialize, Serialize};

use crate::config;

/// Retrieves the block allocation for a specific vehicle.
///
/// # Errors
///
/// Returns an error when the block management API request fails or the
/// response cannot be deserialized.
pub async fn allocation<P>(vehicle_id: &str, provider: &P) -> Result<Option<Allocation>>
where
    P: Config + HttpRequest + Identity,
{
    let url = Config::get(provider, config::BLOCK_MGT_URL).await?;
    let identity = Config::get(provider, config::API_IDENTITY).await?;

    let url = format!("{url}/allocations/vehicles/{vehicle_id}?currentTrip=true");
    let token = Identity::access_token(provider, identity).await?;

    let request = http::Request::builder()
        .method(Method::GET)
        .uri(url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(Empty::<Bytes>::new())
        .context("building allocation_by_vehicle request")?;

    let response = HttpRequest::fetch(provider, request)
        .await
        .context("failed to fetch block allocation for vehicle")?;

    let body = response.into_body();
    let envelope: AllocationResponse =
        serde_json::from_slice(&body).context("Failed to decode allocation response")?;

    Ok(envelope.current.into_iter().next())
}

/// Retrieves the cached block allocation for a specific vehicle.
///
/// # Errors
///
/// Returns an error when the block management API request fails or the
/// response cannot be deserialized.
pub async fn cached_allocation<P>(
    vehicle_id: &str, timestamp: i64, provider: &P,
) -> Result<Option<BlockInstance>>
where
    P: Config + HttpRequest + Identity,
{
    let url = Config::get(provider, config::BLOCK_MGT_URL).await?;
    let identity = Config::get(provider, config::API_IDENTITY).await?;

    let token = Identity::access_token(provider, identity).await?;
    let endpoint = format!(
        "{url}/allocations/vehicles/{vehicle_id}?currentTrip=true&siblings=true&nowUnixTimeSeconds={timestamp}"
    );

    let request = http::Request::builder()
        .uri(&endpoint)
        .method(Method::GET)
        .header(CACHE_CONTROL, "max-age=20") // 20 seconds
        .header(IF_NONE_MATCH, vehicle_id)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .body(Empty::<Bytes>::new())
        .context("building block management request")?;
    let response = HttpRequest::fetch(provider, request).await.context("fetching allocations")?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let body = response.into_body();
    let allocation: Option<BlockInstance> =
        serde_json::from_slice(&body).context("deserializing allocations")?;

    Ok(allocation)
}

/// Retrieves all block allocations.
///
/// # Errors
///
/// Returns an error when the block management API request fails or the
/// response cannot be deserialized.
pub async fn allocations<P>(provider: &P) -> Result<Vec<Allocation>>
where
    P: Config + HttpRequest + Identity,
{
    let url = Config::get(provider, config::BLOCK_MGT_URL).await?;
    let identity = Config::get(provider, config::API_IDENTITY).await?;

    let url = format!("{url}/allocations");
    let token = Identity::access_token(provider, identity).await?;

    let request = http::Request::builder()
        .method(Method::GET)
        .uri(url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(Empty::<Bytes>::new())
        .context("building all_allocations request")?;

    let response = HttpRequest::fetch(provider, request)
        .await
        .context("Block management list request failed")?;

    let body = response.into_body();
    let envelope: AllocationResponse =
        serde_json::from_slice(&body).context("Failed to decode allocations response")?;

    Ok(envelope.all)
}

/// Retrieves the identifiers of the vehicles allocated to the trip with the
/// given external reference.
///
/// # Errors
///
/// Returns an error when the block management API request fails or the
/// response cannot be deserialized.
pub async fn trip_allocations<P>(external_ref_id: &str, provider: &P) -> Result<Vec<String>>
where
    P: Config + HttpRequest + Identity,
{
    let url = Config::get(provider, config::BLOCK_MGT_URL).await?;
    let identity = Config::get(provider, config::API_IDENTITY).await?;

    let token = Identity::access_token(provider, identity).await?;

    let request = http::Request::builder()
        .method(Method::GET)
        .uri(format!("{url}/allocations/trips?externalRefId={external_ref_id}"))
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .body(Empty::<Bytes>::new())
        .context("building trip allocations request")?;

    let response =
        HttpRequest::fetch(provider, request).await.context("fetching trip allocations")?;

    let body = response.into_body();
    let allocated: Vec<String> =
        serde_json::from_slice(&body).context("deserializing trip allocations response")?;

    Ok(allocated)
}

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
struct AllocationResponse {
    current: Vec<Allocation>,
    all: Vec<Allocation>,
}

/// A vehicle's allocation to a block of trips.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Allocation {
    /// The operational block the vehicle is allocated to.
    pub operational_block_id: String,
    /// The trip the vehicle is currently assigned to.
    pub trip_id: String,
    /// The service date of the allocation.
    pub service_date: String,
    /// The scheduled start time of the trip.
    pub start_time: String,
    /// The allocated vehicle's identifier.
    pub vehicle_id: String,
    /// The allocated vehicle's label.
    pub vehicle_label: String,
    /// The route the trip belongs to.
    pub route_id: String,
    /// The direction of travel, if known.
    pub direction_id: Option<u32>,
    /// The external reference identifier for the allocation.
    pub reference_id: String,
    /// The scheduled end time of the trip.
    pub end_time: String,
    /// The current delay in seconds.
    pub delay: i64,
    /// The start of the allocation as a Unix timestamp.
    pub start_datetime: i64,
    /// The end of the allocation as a Unix timestamp.
    pub end_datetime: i64,
    /// Whether the trip has been canceled.
    pub is_canceled: bool,
    /// Whether the allocation is a copy of another allocation.
    pub is_copied: bool,
    /// The timezone of the allocation.
    pub timezone: String,
    /// The time the allocation was created.
    pub creation_datetime: String,
}

/// A single instance of a block with its trip and allocated vehicles.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct BlockInstance {
    /// The trip the block instance belongs to.
    pub trip_id: String,
    /// The scheduled start time of the trip.
    pub start_time: String,
    /// The service date of the block instance.
    pub service_date: String,
    /// The vehicles allocated to the block instance.
    pub vehicle_ids: Vec<String>,
    /// Whether the upstream API reported an error for this instance.
    pub error: bool,
}

impl BlockInstance {
    /// Whether the upstream API reported an error for this instance.
    #[must_use]
    pub const fn has_error(&self) -> bool {
        self.error
    }
}
