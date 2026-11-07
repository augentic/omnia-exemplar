//! Upstream bearer-token authentication through [`Identity::access_token`].
//!
//! Deployment credentials and upstream identities are resolved at use: the
//! identity name is a configuration setting loaded through
//! [`IdentitySettings`], handed to [`Identity::access_token`], and attached
//! as an `Authorization: Bearer <token>` header on the outbound
//! [`HttpRequest::fetch_json`] call.

use anyhow::Context as _;
use axum::Json;
use axum::body::Body;
use axum::extract::Path;
use axum::http::header::AUTHORIZATION;
use axum::http::{Method, Request};
use axum::routing::get;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::{
    Capabilities, Config, HttpRequest, HttpRequestExt as _, Identity, IdentityExt as _, Result,
    Settings,
};
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
    P: Config + HttpRequest + Identity,
{
    let settings = IdentitySettings::load(context.config()).await?;
    let token = context.identity().access_token(&settings.upstream_identity).await?;

    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("{}/status/{resource}", settings.upstream_url.trim_end_matches('/')))
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .context("building upstream status request")?;

    let status = context.http().fetch_json(request).await?;
    Ok(Json(status))
}
