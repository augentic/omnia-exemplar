//! Block Management API client.

use anyhow::Context as _;
use axum::body::Bytes;
use http::Method;
use http::header::{AUTHORIZATION, CACHE_CONTROL, IF_NONE_MATCH};
use omnia_http_cache::HttpCache;
use omnia_sdk::{
    Config, HttpClient, Identity, IdentityExt as _, Result, Settings, StateStore, bad_gateway,
};
use serde::{Deserialize, Serialize};

/// The settings the Block Management client reads.
#[derive(Debug, Settings)]
pub struct BlockMgtSettings {
    // Base URL of the Block Management API.
    block_mgt_url: String,
    // Identity used to acquire access tokens for the operator APIs.
    api_identity: String,
}

// Responses are cached for twenty seconds through the provider's
// `StateStore` under the quoted etag `"allocation:<vehicle_id>"`.
pub(crate) async fn cached_allocation<P>(
    provider: &P, vehicle_id: &str, timestamp: i64,
) -> Result<Option<BlockInstance>>
where
    P: Config + HttpClient + Identity + StateStore,
{
    let BlockMgtSettings {
        block_mgt_url: url,
        api_identity: identity,
    } = BlockMgtSettings::load(provider).await?;

    let token = Identity::named(provider, &identity).access_token().await?;
    let endpoint = format!(
        "{url}/allocations/vehicles/{vehicle_id}?currentTrip=true&siblings=true&nowUnixTimeSeconds={timestamp}"
    );

    // `HttpCache` requires a quoted strong etag.
    let request = http::Request::builder()
        .uri(&endpoint)
        .method(Method::GET)
        .header(CACHE_CONTROL, "max-age=20")
        .header(IF_NONE_MATCH, format!("\"allocation:{vehicle_id}\""))
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .body(Bytes::new())
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
