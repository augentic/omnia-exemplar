//! # Docstore examples
//!
//! The rich `omnia:docstore` showcase: three GTFS-like collections — stops,
//! routes, and stop times — exercising full CRUD and the filter language
//! through combined query endpoints, plus sorting and
//! limit/continuation pagination. Its list queries use optional features
//! beside the portable subset, so a backend that serves only that subset
//! refuses them. Where `capability::document` proves the [`docstore`]
//! interface with one upsert, this crate shows what a document-backed
//! service actually looks like. A create whose id is already stored answers
//! `409 Conflict` through [`stop::CreateError`].
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
//! The crate-level tests drive the stem's component over the in-memory
//! document store, which evaluates every filter over the stored JSON.
//!
//! [`docstore`]: omnia_sdk::docstore

pub mod route;
pub mod stop;
pub mod stop_time;

use omnia_sdk::api::Handlers;

/// The stem's handlers, merged onto the root router.
pub fn handlers() -> Handlers {
    Handlers::from([route::handlers(), stop::handlers(), stop_time::handlers()])
}
