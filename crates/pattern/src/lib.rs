//! # Pattern examples
//!
//! Composition patterns over the Omnia guest capabilities, distilled from
//! real replatformed services. Where `capability` proves one capability at a
//! time, each handler here composes several:
//!
//! - [`decode::DecodeSegmentRequest`] — decode-through-cache: [`StateStore`] miss →
//!   [`decode::DecodeSettings`] (endpoint and client certificate) →
//!   [`HttpClientExt::fetch_json`] → write back through [`StateStore`] with a TTL.
//! - [`cas::UpdateQuotaRequest`] — optimistic concurrency:
//!   [`StateStore::compare_and_swap`] with [`cas::QuotaError::Conflict`]
//!   answering `409 Conflict` through its own [`IntoResponse`] implementation.
//! - [`identity::UpstreamStatusReply`] — bearer-token authentication:
//!   [`identity::IdentitySettings`] for the identity name and endpoint URL →
//!   [`Identity::named`] → [`IdentityExt::access_token`] →
//!   [`HttpClientExt::fetch_json`] with `Authorization: Bearer <token>`.
//! - [`place::UpsertPlaceRequest`] — relational upsert through [`SqlDb`] using
//!   parameterized `INSERT … ON CONFLICT` SQL.
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
//! [`FromRow`]: omnia_sdk::sql::FromRow
//! [`HttpClientExt::fetch_json`]: omnia_sdk::HttpClientExt::fetch_json
//! [`IdentityExt::access_token`]: omnia_sdk::IdentityExt::access_token
//! [`IntoResponse`]: axum::response::IntoResponse

pub mod cas;
pub mod decode;
pub mod identity;
pub mod place;
mod state_keys;

use omnia_sdk::api::Handlers;
use omnia_sdk::{Config, HttpClient, Identity, SqlDb, StateStore};

/// The capabilities the stem's handlers reach, implemented by every provider that has them.
pub trait Requires:
    Config + HttpClient + Identity + SqlDb + StateStore + Send + Sync + 'static
{
}

impl<P> Requires for P where
    P: Config + HttpClient + Identity + SqlDb + StateStore + Send + Sync + 'static
{
}

/// The stem's handlers, merged onto the root routers.
pub fn handlers<P: Requires>() -> Handlers<P> {
    Handlers::from([cas::handlers(), decode::handlers(), identity::handlers(), place::handlers()])
}
