//! Upstream bearer-token authentication through [`Identity::access_token`].
//!
//! Deployment credentials and upstream identities are resolved at use: the
//! identity name is a configuration setting read through [`Config::get`],
//! handed to [`Identity::access_token`], and attached as an `Authorization:
//! Bearer <token>` header on the outbound [`HttpRequest::fetch`] call.

use anyhow::Context as _;
use axum::Json;
use axum::body::Body;
use axum::extract::Path;
use axum::http::header::AUTHORIZATION;
use axum::http::{Method, Request};
use axum::routing::get;
use common::Capabilities;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::{Config, HttpRequest, Identity, Result, bad_gateway};
use serde::{Deserialize, Serialize};

pub(crate) const UPSTREAM_STATUS: &str = "/examples/patterns/upstream/{resource}";

// Config key naming the upstream service base URL.
pub(crate) const UPSTREAM_URL: &str = "PATTERN_UPSTREAM_URL";

// Config key naming the identity whose access token authenticates upstream requests.
pub(crate) const UPSTREAM_IDENTITY: &str = "PATTERN_UPSTREAM_IDENTITY";

pub(crate) fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::new().route(UPSTREAM_STATUS, get(fetch_upstream_status))
}

/// Status payload returned by the upstream service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpstreamStatusReply {
    /// Resource identifier.
    pub resource: String,
    /// Upstream status string.
    pub status: String,
}

pub(crate) async fn fetch_upstream_status<P>(
    context: Context<P>, Path(resource): Path<String>,
) -> Result<Json<UpstreamStatusReply>>
where
    P: Config + HttpRequest + Identity + 'static,
{
    let provider = context.provider();
    let base_url = Config::get(provider, UPSTREAM_URL).await?;
    let identity = Config::get(provider, UPSTREAM_IDENTITY).await?;
    let token = Identity::access_token(provider, identity).await?;

    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("{}/status/{resource}", base_url.trim_end_matches('/')))
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .context("building upstream status request")?;

    let response = HttpRequest::fetch(provider, request).await?;
    if !response.status().is_success() {
        return Err(bad_gateway!("upstream returned {}", response.status()));
    }

    let body = response.into_body();
    let status = serde_json::from_slice(&body).context("parsing upstream status response")?;
    Ok(Json(status))
}
