//! # Docstore examples
//!
//! The rich `wasi:docstore` showcase: three GTFS-like collections — stops,
//! routes, and stop times — exercising full CRUD and every portable filter
//! type through combined query endpoints, plus sorting and
//! limit/continuation pagination. Where `capability::document`
//! proves the [`DocumentStore`] capability with one upsert, this crate
//! shows what a document-backed service actually looks like.
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
//! Each handler is generic over `P: DocumentStore`, so the same code runs
//! inside the WASM guest (served by [`router`]) and against the
//! filter-evaluating `MemoryDocs` double in the crate-level tests.
//!
//! [`DocumentStore`]: omnia_sdk::DocumentStore

pub mod route;
pub mod stop;
pub mod stop_time;

use common::Capabilities;
use omnia_sdk::api::http::Router;
use omnia_sdk::api::messaging;

/// The stem's routes, merged onto the root router.
pub fn router<P: Capabilities>() -> Router<P> {
    common::routers([
        // one entry per part with routes, each added at the end
        route::routes(),
        stop::routes(),
        stop_time::routes(),
    ])
}

/// The stem's topics, merged onto the root messaging router.
pub fn topics<P: Capabilities>() -> messaging::Routes<P> {
    common::topics([
        // one entry per part with topics, each added at the end
    ])
}
