//! Upstream bearer-token authentication through [`Identity::access_token`].
//!
//! Deployment credentials and upstream identities are resolved at use: the
//! identity name is a configuration setting read through [`Config::get`],
//! handed to [`Identity::access_token`], and attached as an `Authorization:
//! Bearer <token>` header on the outbound [`HttpRequest::fetch`] call.

use anyhow::Context as _;
use axum::body::Body;
use common::Capabilities;
use http::Method;
use http::header::AUTHORIZATION;
use omnia_sdk::api::http::get;
use omnia_sdk::api::{Client, Context};
use omnia_sdk::{Config, HttpRequest, Identity, Result, bad_gateway};
use serde::{Deserialize, Serialize};

/// Upstream status check route.
pub const UPSTREAM_STATUS: &str = "/examples/patterns/upstream/{resource}";

/// Config key naming the upstream service base URL.
pub const UPSTREAM_URL: &str = "PATTERN_UPSTREAM_URL";

/// Config key naming the identity whose access token authenticates upstream requests.
pub const UPSTREAM_IDENTITY: &str = "PATTERN_UPSTREAM_IDENTITY";

pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {
    axum::Router::new().route(UPSTREAM_STATUS, get(fetch_upstream_status))
}

/// Request the status of an upstream resource.
#[derive(Debug, Clone, Deserialize)]
pub struct UpstreamStatusRequest {
    /// Resource identifier (path parameter).
    pub resource: String,
}

/// Status payload returned by the upstream service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpstreamStatusReply {
    /// Resource identifier.
    pub resource: String,
    /// Upstream status string.
    pub status: String,
}

/// Reads the upstream URL and identity name from [`Config`], acquires a
/// bearer token from [`Identity::access_token`], and fetches the upstream
/// resource status.
///
/// # Errors
///
/// Returns an error when configuration or token acquisition fails, the
/// upstream request fails or returns a non-success status (`502`), or the
/// response body cannot be deserialized.
pub async fn fetch_upstream_status<P>(
    input: UpstreamStatusRequest, context: Context<P>,
) -> Result<UpstreamStatusReply>
where
    P: Config + HttpRequest + Identity,
{
    let provider = context.provider();
    let base_url = Config::get(provider, UPSTREAM_URL).await?;
    let identity = Config::get(provider, UPSTREAM_IDENTITY).await?;
    let token = Identity::access_token(provider, identity).await?;

    let request = http::Request::builder()
        .method(Method::GET)
        .uri(format!("{}/status/{}", base_url.trim_end_matches('/'), input.resource))
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .context("building upstream status request")?;

    let response = HttpRequest::fetch(provider, request).await?;
    if !response.status().is_success() {
        return Err(bad_gateway!("upstream returned {}", response.status()));
    }

    let body = response.into_body();
    serde_json::from_slice(&body).context("parsing upstream status response").map_err(Into::into)
}
