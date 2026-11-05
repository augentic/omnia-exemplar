//! # Pattern examples
//!
//! Composition patterns over the Omnia guest capabilities, distilled from
//! real replatformed services. Where `capability` proves one capability at a
//! time, each handler here composes several:
//!
//! - [`DecodeSegmentRequest`] — decode-through-cache: [`StateStore`] miss →
//!   [`Config`] (endpoint and client certificate) → [`HttpRequest`] → write
//!   back through [`StateStore`] with a TTL.
//! - [`UpdateQuotaRequest`] — optimistic concurrency: [`StateStore::cas`]
//!   with [`QuotaError::Conflict`] answering `409 Conflict` through its
//!   [`HttpError`] conversion.
//! - [`UpstreamStatusRequest`] — bearer-token authentication: [`Config::get`]
//!   for the identity name and endpoint URL → [`Identity::access_token`] →
//!   [`HttpRequest::fetch`] with `Authorization: Bearer <token>`.
//! - [`UpsertPlaceRequest`] — relational upsert through [`TableStore`] using
//!   parameterized `INSERT … ON CONFLICT` SQL.
//! - [`NearbyPlacesRequest`] — a radius query as a bounding-box `SELECT`
//!   over [`common::RowExt`] refined by a haversine check, replacing the
//!   "geospatial index bolted onto a KV store" anti-pattern.
//! - [`PlaceError`] — a structured JSON error body: the upsert handler owns
//!   its error type, and the [`HttpError`] conversion serializes it as
//!   `application/json` extending the framework's fixed `error` / `message`
//!   `ErrorBody` envelope with the variant's domain fields.
//!
//! [`Config`]: omnia_sdk::Config
//! [`Config::get`]: omnia_sdk::Config::get
//! [`HttpError`]: omnia_sdk::HttpError
//! [`HttpRequest`]: omnia_sdk::HttpRequest
//! [`HttpRequest::fetch`]: omnia_sdk::HttpRequest::fetch
//! [`Identity::access_token`]: omnia_sdk::Identity::access_token
//! [`StateStore`]: omnia_sdk::StateStore
//! [`StateStore::cas`]: omnia_sdk::StateStore::cas
//! [`TableStore`]: omnia_sdk::TableStore

pub mod cas;
pub mod decode;
pub mod identity;
pub mod place;

pub use cas::{QUOTA, QuotaError, QuotaRecord, UpdateQuotaRequest, quota_key, update_quota};
use common::Capabilities;
pub use decode::{DECODE, DecodeSegmentReply, DecodeSegmentRequest, Segment, decode_segment};
pub use identity::{
    UPSTREAM_IDENTITY, UPSTREAM_STATUS, UPSTREAM_URL, UpstreamStatusReply, UpstreamStatusRequest,
    fetch_upstream_status,
};
use omnia_sdk::api::Client;
pub use place::{
    NEARBY, NearbyPlace, NearbyPlacesReply, NearbyPlacesRequest, PLACES, Place, PlaceError,
    UpsertPlaceReply, UpsertPlaceRequest, nearby_places, upsert_place,
};

/// The stem's routes, merged onto the root router.
pub fn router<P: Capabilities>() -> axum::Router<Client<P>> {
    common::routers([
        // one entry per part with routes, each added at the end
        cas::routes(),
        decode::routes(),
        identity::routes(),
        place::routes(),
    ])
}
