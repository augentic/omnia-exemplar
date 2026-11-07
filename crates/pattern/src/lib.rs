//! # Pattern examples
//!
//! Composition patterns over the Omnia guest capabilities, distilled from
//! real replatformed services. Where `capability` proves one capability at a
//! time, each handler here composes several:
//!
//! - [`decode::DecodeSegmentRequest`] — decode-through-cache: [`StateStore`] miss →
//!   [`decode::DecodeSettings`] (endpoint and client certificate) →
//!   [`HttpRequest::fetch_json`] → write back through [`StateStore`] with a TTL.
//! - [`cas::UpdateQuotaRequest`] — optimistic concurrency: [`StateStore::cas`]
//!   with [`cas::QuotaError::Conflict`] answering `409 Conflict` through its
//!   own [`IntoResponse`] implementation.
//! - [`identity::UpstreamStatusReply`] — bearer-token authentication:
//!   [`identity::IdentitySettings`] for the identity name and endpoint URL →
//!   [`Identity::access_token`] → [`HttpRequest::fetch_json`] with
//!   `Authorization: Bearer <token>`.
//! - [`place::UpsertPlaceRequest`] — relational upsert through [`TableStore`] using
//!   parameterized `INSERT … ON CONFLICT` SQL.
//! - [`place::NearbyPlacesRequest`] — a radius query as a bounding-box `SELECT`
//!   into [`FromRow`] records refined by a haversine check, replacing the
//!   "geospatial index bolted onto a KV store" anti-pattern.
//! - [`place::PlaceError`] — a structured JSON error body: the upsert handler owns
//!   its error type, and its [`IntoResponse`] implementation serializes it as
//!   `application/json` extending the framework's fixed `error` / `message`
//!   `ErrorBody` envelope with the variant's domain fields.
//!
//! [`FromRow`]: omnia_sdk::sql::FromRow
//! [`HttpRequest::fetch_json`]: omnia_sdk::HttpRequest::fetch_json
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
use omnia_sdk::api::Handlers;

/// The stem's handlers, merged onto the root routers.
pub fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::from([cas::handlers(), decode::handlers(), identity::handlers(), place::handlers()])
}
