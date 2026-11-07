//! Decode-through-cache example.
//!
//! The canonical capability-composition vignette: check [`StateStore`] for a
//! previously decoded segment, and on a miss resolve the decoder endpoint
//! and client certificate from [`Config`], call the decoder through
//! [`HttpRequest`], then write the result back through [`StateStore`] with a
//! TTL. Credentials never leave configuration: the certificate travels as a
//! request header, so outbound HTTP stays generic and no TLS-specific
//! capability is needed.

use anyhow::Context as _;
use axum::Json;
use axum::body::Body;
use axum::http::{Method, Request};
use axum::routing::post;
use common::Capabilities;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::{
    Config, HttpRequest, HttpRequestExt as _, Result, Settings, StateStore, StateStoreExt as _,
};
use serde::{Deserialize, Serialize};

pub(crate) const DECODE: &str = "/examples/patterns/decode";

// Cache lifetime for decoded segments, in seconds.
pub(crate) const SEGMENT_TTL_SECS: u64 = 24 * 60 * 60;

/// The settings the decoder client reads.
#[derive(Debug, Settings)]
#[setting(prefix = "PATTERN_")]
pub struct DecodeSettings {
    // The decoder endpoint URL.
    decoder_url: String,
    // The client certificate forwarded to the decoder.
    client_cert: String,
}

pub(crate) fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::new().route(DECODE, post(decode_segment))
}

pub(crate) fn segment_key(code: &str) -> String {
    format!("pattern:segment:{code}")
}

/// A decoded segment as returned by the upstream decoder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    /// Identifier echoed back by the decoder.
    pub code: String,
    /// Decoded geometry as `[lat, lon]` pairs.
    pub points: Vec<[f64; 2]>,
}

/// Decode a segment code, consulting the cache first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodeSegmentRequest {
    /// Opaque code understood by the decoder service.
    pub code: String,
}

/// Decoded segment plus where it came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodeSegmentReply {
    /// Whether the segment was served from the cache.
    pub cached: bool,
    /// The decoded segment.
    pub segment: Segment,
}

pub(crate) async fn decode_segment<P>(
    context: Context<P>, Json(input): Json<DecodeSegmentRequest>,
) -> Result<Json<DecodeSegmentReply>>
where
    P: Config + HttpRequest + StateStore + 'static,
{
    let key = segment_key(&input.code);

    // Cache hit: no config read, no outbound request.
    if let Some(segment) = context.state().get_json(&key).await? {
        return Ok(Json(DecodeSegmentReply {
            cached: true,
            segment,
        }));
    }

    // Cache miss: endpoint and credential material both come from
    // config; the certificate rides an ordinary header so the HTTP
    // capability stays generic.
    let settings = DecodeSettings::load(context.config()).await?;

    let body = serde_json::to_vec(&serde_json::json!({ "code": input.code }))
        .context("serializing decode request")?;
    let request = Request::builder()
        .method(Method::POST)
        .uri(settings.decoder_url)
        .header("Content-Type", "application/json")
        .header("Client-Cert", settings.client_cert)
        .body(Body::from(body))
        .context("building decode request")?;

    let segment: Segment = context.http().fetch_json(request).await?;

    // Write back through the cache so the next lookup short-circuits.
    context.state().set_json(&key, &segment, Some(SEGMENT_TTL_SECS)).await?;

    Ok(Json(DecodeSegmentReply {
        cached: false,
        segment,
    }))
}
