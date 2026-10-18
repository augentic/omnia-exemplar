//! # Pattern examples
//!
//! Composition patterns over the Omnia guest capabilities, distilled from
//! real replatformed services. Where `capability-examples` proves one
//! capability at a time, each handler here composes several:
//!
//! - [`DecodeSegmentRequest`] — decode-through-cache: [`StateStore`] miss →
//!   [`Config`] (endpoint and client certificate) → [`HttpRequest`] → write
//!   back through [`StateStore`] with a TTL.
//! - [`UpsertPlaceRequest`] — relational upsert through [`TableStore`] using
//!   the guest ORM (`entity!` + `InsertBuilder` with an `ON CONFLICT`
//!   target).
//! - [`NearbyPlacesRequest`] — a radius query as a bounding-box `SELECT`
//!   refined by a haversine check, replacing the "geospatial index bolted
//!   onto a KV store" anti-pattern.
//! - [`PlaceError`] — a structured JSON error body: the upsert handler owns
//!   its error type, and the `HttpError` conversion serializes it as
//!   `application/json` extending the framework's fixed `error` / `message`
//!   `ErrorBody` envelope with the variant's domain fields.
//!
//! The crate-level tests drive every handler through `omnia_test::guest::Provider`
//! doubles whose `MatchedHttp` records outbound requests, and [`router`]
//! serves them under `/examples/patterns/*` so the default WASM capability
//! implementations are instantiated in a real guest.
//!
//! [`Config`]: omnia_sdk::Config
//! [`HttpRequest`]: omnia_sdk::HttpRequest
//! [`StateStore`]: omnia_sdk::StateStore
//! [`TableStore`]: omnia_sdk::TableStore

pub mod decode;
pub mod place;
pub mod routes;

use axum::Json;
use axum::response::{IntoResponse, Response};
pub use decode::{DecodeSegmentReply, DecodeSegmentRequest, Segment, decode_segment};
use omnia_sdk::api::http::{MethodFilter, RawRequest, handle_with, post};
use omnia_sdk::api::{Client, DecodeError};
use omnia_sdk::{Config, HttpRequest, StateStore, TableStore};
pub use place::{
    NearbyPlace, NearbyPlacesReply, NearbyPlacesRequest, Place, PlaceError, UpsertPlaceReply,
    UpsertPlaceRequest, nearby_places, upsert_place,
};

/// The crate's routes, merged onto the guest's router.
///
/// The nearby query is a GET whose request is its body, so it carries its
/// own codec through `handle_with`; the default `get` codec reads the query
/// string alone.
pub fn router<P>() -> axum::Router<Client<P>>
where
    P: Config + HttpRequest + StateStore + TableStore + Send + Sync + 'static,
{
    axum::Router::new()
        .route(routes::DECODE, post(decode_segment))
        .route(routes::PLACES, post(upsert_place))
        .route(
            routes::NEARBY,
            handle_with(
                MethodFilter::GET,
                nearby_places,
                |raw: RawRequest<'_>| decode_nearby(raw.body),
                encode_nearby,
            ),
        )
}

// Demonstration only: this does exactly what the built-in `post` codec does,
// and exists because the route is a GET. A route with an ordinary JSON body
// uses `post(handler)` and no codec of its own.
fn decode_nearby(body: &[u8]) -> Result<NearbyPlacesRequest, DecodeError> {
    serde_json::from_slice(body)
        .map_err(|error| DecodeError::new(format!("malformed JSON body: {error}")))
}

// `handle_with` requires both halves of the codec; this is the default
// JSON encoding.
fn encode_nearby(reply: NearbyPlacesReply) -> Response {
    Json(reply).into_response()
}
