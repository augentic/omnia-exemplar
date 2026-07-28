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
use bytes::Bytes;
use http::Method;
use http_body_util::Full;
use omnia_guest::api::{CallContext, Operation, Provider};
use omnia_guest::{Config, Error, HttpRequest, Result, StateStore, bad_gateway};
use serde::{Deserialize, Serialize};

/// Config key naming the decoder endpoint URL.
pub const DECODER_URL: &str = "PATTERN_DECODER_URL";

/// Config key holding the client certificate forwarded to the decoder.
pub const CLIENT_CERT: &str = "PATTERN_CLIENT_CERT";

/// Cache lifetime for decoded segments, in seconds.
pub const SEGMENT_TTL_SECS: u64 = 24 * 60 * 60;

/// State-store key for a decoded segment.
#[must_use]
pub fn segment_key(code: &str) -> String {
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
#[derive(Debug, Clone, Deserialize)]
pub struct DecodeSegmentRequest {
    /// Opaque code understood by the decoder service.
    pub code: String,
}

/// Decoded segment plus where it came from.
#[derive(Debug, Clone, Serialize)]
pub struct DecodeSegmentReply {
    /// Whether the segment was served from the cache.
    pub cached: bool,
    /// The decoded segment.
    pub segment: Segment,
}

impl<P> Operation<P> for DecodeSegmentRequest
where
    P: Provider + Config + HttpRequest + StateStore,
{
    type Error = Error;
    type Input = Self;
    type Output = DecodeSegmentReply;

    async fn call(input: Self, context: CallContext<'_, P>) -> Result<DecodeSegmentReply> {
        let provider = context.provider;
        let key = segment_key(&input.code);

        // Cache hit: no config read, no outbound request.
        if let Some(bytes) = StateStore::get(provider, &key).await? {
            let segment =
                serde_json::from_slice(&bytes).context("parsing cached segment")?;
            return Ok(DecodeSegmentReply {
                cached: true,
                segment,
            });
        }

        // Cache miss: endpoint and credential material both come from
        // config; the certificate rides an ordinary header so the HTTP
        // capability stays generic.
        let url = Config::get(provider, DECODER_URL).await?;
        let cert = Config::get(provider, CLIENT_CERT).await?;

        let body = serde_json::to_vec(&serde_json::json!({ "code": input.code }))
            .context("serializing decode request")?;
        let request = http::Request::builder()
            .method(Method::POST)
            .uri(url)
            .header("Content-Type", "application/json")
            .header("Client-Cert", cert)
            .body(Full::new(Bytes::from(body)))
            .context("building decode request")?;

        let response = HttpRequest::fetch(provider, request).await?;
        if !response.status().is_success() {
            return Err(bad_gateway!("decoder returned {}", response.status()));
        }

        let body = response.into_body();
        let segment: Segment =
            serde_json::from_slice(&body).context("parsing decoder response")?;

        // Write back through the cache so the next lookup short-circuits.
        let bytes = serde_json::to_vec(&segment).context("serializing segment for cache")?;
        StateStore::set(provider, &key, &bytes, Some(SEGMENT_TTL_SECS)).await?;

        Ok(DecodeSegmentReply {
            cached: false,
            segment,
        })
    }
}
