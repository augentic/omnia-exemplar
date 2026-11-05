//! Block Management API client.

use anyhow::Context as _;
use axum::body::Body;
use http::Method;
use http::header::{AUTHORIZATION, CACHE_CONTROL, IF_NONE_MATCH};
use omnia_http_cache::HttpCache;
use omnia_sdk::{Config, HttpRequest, Identity, Result, StateStore, bad_gateway};
use serde::{Deserialize, Serialize};

// Base URL of the Block Management API.
pub(crate) const BLOCK_MGT_URL: &str = "BLOCK_MGT_URL";

// Identity used to acquire access tokens for the operator APIs.
pub(crate) const API_IDENTITY: &str = "API_IDENTITY";

// Responses are cached for twenty seconds through the provider's
// `StateStore` under the quoted, namespaced etag `"allocation:<vehicle_id>"`.
pub(crate) async fn cached_allocation<P>(
    vehicle_id: &str, timestamp: i64, provider: &P,
) -> Result<Option<BlockInstance>>
where
    P: Config + HttpRequest + Identity + StateStore,
{
    let url = Config::get(provider, BLOCK_MGT_URL).await?;
    let identity = Config::get(provider, API_IDENTITY).await?;

    let token = Identity::access_token(provider, identity).await?;
    let endpoint = format!(
        "{url}/allocations/vehicles/{vehicle_id}?currentTrip=true&siblings=true&nowUnixTimeSeconds={timestamp}"
    );

    // `HttpCache` requires a quoted strong etag; the prefix keeps the cache
    // key out of the guest's own state-store namespace.
    let request = http::Request::builder()
        .uri(&endpoint)
        .method(Method::GET)
        .header(CACHE_CONTROL, "max-age=20")
        .header(IF_NONE_MATCH, format!("\"allocation:{vehicle_id}\""))
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .context("building block management request")?;
    let response =
        HttpCache::new(provider, provider).fetch(request).await.context("fetching allocations")?;

    if !response.status().is_success() {
        return Err(bad_gateway!("Block Management API returned {}", response.status()));
    }

    let body = response.into_body();
    let allocation: Option<BlockInstance> =
        serde_json::from_slice(&body).context("deserializing allocations")?;

    Ok(allocation)
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
