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
//! - `eq`, `gte`, `lte`, `contains` — [`ListStopsRequest`] query params
//! - `ne` — [`ListStopsRequest::exclude_zone`] (the direct `Ne` codepath)
//! - `eq` + `is_not_null` — [`ListStopsRequest::accessible`]
//! - `is_null` — [`ListStopsRequest::top_level`]
//! - `on_date` — [`ListStopsRequest::updated_on`]
//! - `or(contains, contains)` — [`ListRoutesRequest::q`]
//! - `in_list` — [`ListRoutesRequest::types`]
//! - `negate` — [`ListRoutesRequest::exclude_type`]
//! - `negate(and(...))` — [`ListRoutesRequest::not_agency`] +
//!   [`ListRoutesRequest::not_type`] (De Morgan negation)
//! - `gte`/`lte` ranges — [`ListStopTimesRequest`] time and sequence bounds
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
use omnia_sdk::api::Client;

pub use crate::route::{
    CreateRouteRequest, GetRouteRequest, ListRoutesRequest, ROUTE, ROUTES, Route, RoutesReply,
    create_route, get_route, list_routes,
};
pub use crate::stop::{
    CreateStopRequest, DeleteStopReply, DeleteStopRequest, DocumentRecord, GetStopRequest,
    ListStopsRequest, STOP, STOPS, Stop, StopsReply, UpsertStopRequest, create_stop, delete_stop,
    get_stop, list_stops, upsert_stop,
};
pub use crate::stop_time::{
    CreateStopTimeRequest, GetStopTimeRequest, ListStopTimesRequest, STOP_TIME, STOP_TIMES,
    StopTime, StopTimesReply, create_stop_time, get_stop_time, list_stop_times,
};

/// The stem's routes, merged onto the root router.
pub fn router<P: Capabilities>() -> axum::Router<Client<P>> {
    common::routers([
        // one entry per part with routes, each added at the end
        route::routes(),
        stop::routes(),
        stop_time::routes(),
    ])
}
