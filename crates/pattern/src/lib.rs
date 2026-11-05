//! # Pattern examples
//!
//! Composition patterns over the Omnia guest capabilities, distilled from
//! real replatformed services. Where `capability` proves one capability at a
//! time, each handler here composes several:
//!
//! - [`decode::DecodeSegmentRequest`] — decode-through-cache: [`StateStore`] miss →
//!   [`Config`] (endpoint and client certificate) → [`HttpRequest`] → write
//!   back through [`StateStore`] with a TTL.
//! - [`cas::UpdateQuotaRequest`] — optimistic concurrency: [`StateStore::cas`]
//!   with [`cas::QuotaError::Conflict`] answering `409 Conflict` through its
//!   own [`IntoResponse`] implementation.
//! - [`identity::UpstreamStatusReply`] — bearer-token authentication: [`Config::get`]
//!   for the identity name and endpoint URL → [`Identity::access_token`] →
//!   [`HttpRequest::fetch`] with `Authorization: Bearer <token>`.
//! - [`place::UpsertPlaceRequest`] — relational upsert through [`TableStore`] using
//!   parameterized `INSERT … ON CONFLICT` SQL.
//! - [`place::NearbyPlacesRequest`] — a radius query as a bounding-box `SELECT`
//!   over [`common::RowExt`] refined by a haversine check, replacing the
//!   "geospatial index bolted onto a KV store" anti-pattern.
//! - [`place::PlaceError`] — a structured JSON error body: the upsert handler owns
//!   its error type, and its [`IntoResponse`] implementation serializes it as
//!   `application/json` extending the framework's fixed `error` / `message`
//!   `ErrorBody` envelope with the variant's domain fields.
//!
//! [`Config`]: omnia_sdk::Config
//! [`Config::get`]: omnia_sdk::Config::get
//! [`HttpRequest`]: omnia_sdk::HttpRequest
//! [`HttpRequest::fetch`]: omnia_sdk::HttpRequest::fetch
//! [`Identity::access_token`]: omnia_sdk::Identity::access_token
//! [`IntoResponse`]: axum::response::IntoResponse
//! [`StateStore`]: omnia_sdk::StateStore
//! [`StateStore::cas`]: omnia_sdk::StateStore::cas
//! [`TableStore`]: omnia_sdk::TableStore

pub mod cas;
pub mod decode;
pub mod identity;
pub mod place;

use common::Capabilities;
use omnia_sdk::api::Client;

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
