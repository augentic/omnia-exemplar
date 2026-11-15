//! Upstream bearer-token authentication through [`IdentityExt::access_token`].
//!
//! Deployment credentials and upstream identities are resolved at use: the
//! identity name is a configuration setting loaded through
//! [`IdentitySettings`] and selected with [`Identity::named`]. Its token is
//! attached as an `Authorization: Bearer <token>` header on the outbound
//! [`HttpClientExt::fetch_json`] call.

use anyhow::Context as _;
use axum::Json;
use axum::body::Bytes;
use axum::extract::Path;
use axum::http::header::AUTHORIZATION;
use axum::http::{Method, Request};
use axum::routing::get;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::{Config, HttpClient, HttpClientExt, Identity, IdentityExt, Result, Settings};
use serde::{Deserialize, Serialize};

pub(crate) const UPSTREAM_STATUS: &str = "/examples/patterns/upstream/{resource}";

/// The settings the upstream status client reads.
#[derive(Debug, Settings)]
#[setting(prefix = "PATTERN_")]
pub struct IdentitySettings {
    // The upstream service base URL.
    upstream_url: String,
    // The identity whose access token authenticates upstream requests.
    upstream_identity: String,
}

pub(crate) fn handlers<P: crate::Requires>() -> Handlers<P> {
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
    P: Config + HttpClient + Identity,
{
    let settings = IdentitySettings::load(context.config()).await?;
    let token = context.identity().named(&settings.upstream_identity).access_token().await?;

    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("{}/status/{resource}", settings.upstream_url.trim_end_matches('/')))
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .body(Bytes::new())
        .context("building upstream status request")?;

    let status = context.http().fetch_json(request).await?;
    Ok(Json(status))
}
