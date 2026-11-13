//! # Docstore examples
//!
//! The rich `wasi:docstore` showcase: three GTFS-like collections — stops,
//! routes, and stop times — exercising full CRUD and every portable filter
//! type through combined query endpoints, plus sorting and
//! limit/continuation pagination. Where `capability::document`
//! proves the [`DocumentDb`] capability with one upsert, this crate
//! shows what a document-backed service actually looks like. A create
//! whose id is already stored answers `409 Conflict` through
//! [`stop::CreateError`].
//!
//! Filter coverage across the three list handlers:
//!
//! - `eq`, `gte`, `lte`, `contains` — [`stop::ListStopsRequest`] query params
//! - `ne` — [`stop::ListStopsRequest::exclude_zone`] (the direct `Ne` codepath)
//! - `eq` + `is_not_null` — [`stop::ListStopsRequest::accessible`]
//! - `is_null` — [`stop::ListStopsRequest::top_level`]
//! - `on_date` — [`stop::ListStopsRequest::updated_on`]
//! - `or(contains, contains)` — [`route::ListRoutesRequest::q`]
//! - `in_list` — [`route::ListRoutesRequest::types`]
//! - `negate` — [`route::ListRoutesRequest::exclude_type`]
//! - `negate(and(...))` — [`route::ListRoutesRequest::not_agency`] +
//!   [`route::ListRoutesRequest::not_type`] (De Morgan negation)
//! - `gte`/`lte` ranges — [`stop_time::ListStopTimesRequest`] time and sequence bounds
//!
//! Each handler is generic over `P: DocumentDb`, so the same code runs
//! inside the WASM guest (served by [`handlers`]) and against the
//! filter-evaluating `MemoryDocs` double in the crate-level tests.

pub mod route;
pub mod stop;
pub mod stop_time;

use omnia_sdk::DocumentDb;
use omnia_sdk::api::Handlers;

/// The capabilities the stem's handlers reach, implemented by every provider that has them.
pub trait Requires: DocumentDb + Send + Sync + 'static {}

impl<P> Requires for P where P: DocumentDb + Send + Sync + 'static {}

/// The stem's handlers, merged onto the root routers.
pub fn handlers<P: Requires>() -> Handlers<P> {
    Handlers::from([route::handlers(), stop::handlers(), stop_time::handlers()])
}
