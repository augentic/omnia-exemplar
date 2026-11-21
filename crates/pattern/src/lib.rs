//! # Pattern examples
//!
//! Composition patterns over the Omnia host interfaces, distilled from real
//! replatformed services. Where `capability` proves one interface at a time,
//! each handler here composes several:
//!
//! - [`decode::DecodeSegmentRequest`] — decode-through-cache: a `default`
//!   bucket miss → [`decode::DecodeSettings`] (endpoint and client
//!   certificate) → [`http::post`] → write back with
//!   [`Bucket::set_json`] and a TTL.
//! - [`cas::UpdateQuotaRequest`] — optimistic concurrency:
//!   [`Bucket::update_json`] checks the caller's expected version against
//!   every record it reads, answering `409 Conflict` with [`conflict!`].
//! - [`identity::UpstreamStatusReply`] — bearer-token authentication:
//!   [`identity::IdentitySettings`] for the identity name and endpoint URL →
//!   [`omnia_sdk::identity::open`] → [`Identity::access_token`] →
//!   [`http::get`] with `Authorization: Bearer <token>`.
//! - [`place::UpsertPlaceRequest`] — relational upsert on the `default`
//!   [`Database`] using parameterized `INSERT … ON CONFLICT` SQL.
//! - [`place::NearbyPlacesRequest`] — a radius query as a bounding-box `SELECT`
//!   into [`FromRow`] records refined by a haversine check, replacing the
//!   "geospatial index bolted onto a KV store" anti-pattern.
//! - [`place::PlaceError`] — a structured JSON error body: the upsert handler owns
//!   its error type, and its [`IntoResponse`] implementation serializes it as
//!   `application/json` extending the framework's fixed `error` / `message`
//!   `ErrorBody` envelope with the variant's domain fields.
//!
//! A tenant or segment code that is not a portable state key segment is
//! answered `400 Bad Request` before any state is read.
//!
//! [`Bucket::set_json`]: omnia_sdk::keyvalue::Bucket::set_json
//! [`Bucket::update_json`]: omnia_sdk::keyvalue::Bucket::update_json
//! [`conflict!`]: omnia_sdk::conflict
//! [`Database`]: omnia_sdk::sql::Database
//! [`FromRow`]: omnia_sdk::sql::FromRow
//! [`http::get`]: omnia_sdk::http::get
//! [`http::post`]: omnia_sdk::http::post
//! [`Identity::access_token`]: omnia_sdk::identity::Identity::access_token
//! [`IntoResponse`]: axum::response::IntoResponse

pub mod cas;
pub mod decode;
pub mod identity;
pub mod place;
mod state_keys;

use omnia_sdk::Handlers;

/// The stem's handlers, merged onto the root router.
pub fn handlers() -> Handlers {
    Handlers::from([cas::handlers(), decode::handlers(), identity::handlers(), place::handlers()])
}
