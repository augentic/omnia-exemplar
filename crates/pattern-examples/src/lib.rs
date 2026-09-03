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
//!   `application/json`, matching the success content type instead of the
//!   default plain-text error body.
//!
//! The crate-level tests drive every handler through `omnia_test::provider!`
//! doubles whose `MatchedHttp` records outbound requests, and the guest routes them under
//! `/examples/patterns/*` so the default WASM capability implementations are
//! instantiated in a real guest.
//!
//! [`Config`]: omnia_guest::Config
//! [`HttpRequest`]: omnia_guest::HttpRequest
//! [`StateStore`]: omnia_guest::StateStore
//! [`TableStore`]: omnia_guest::TableStore

pub mod decode;
pub mod place;
pub mod routes;

pub use decode::{DecodeSegmentReply, DecodeSegmentRequest, Segment};
pub use place::{
    NearbyPlace, NearbyPlacesReply, NearbyPlacesRequest, Place, PlaceError, UpsertPlaceReply,
    UpsertPlaceRequest,
};
