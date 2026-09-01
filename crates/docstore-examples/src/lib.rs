//! # Docstore examples
//!
//! The rich `wasi:docstore` showcase: three GTFS-like collections — stops,
//! routes, and stop times — exercising full CRUD and every portable filter
//! type through combined query endpoints, plus sorting and
//! limit/continuation pagination. Where `capability_examples::document`
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
//! inside the WASM guest (wired under the [`paths`] constants) and against
//! the filter-evaluating mock provider in the crate-level tests.
//!
//! [`DocumentStore`]: omnia_guest::DocumentStore

pub mod paths;
pub mod route;
pub mod stop;
pub mod stop_time;

use anyhow::Context as _;
use omnia_guest::document_store::Document;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub use crate::route::{
    CreateRouteRequest, GetRouteRequest, ListRoutesRequest, Route, RoutesReply,
};
pub use crate::stop::{
    CreateStopRequest, DeleteStopReply, DeleteStopRequest, GetStopRequest, ListStopsRequest, Stop,
    StopsReply, UpsertStopRequest,
};
pub use crate::stop_time::{
    CreateStopTimeRequest, GetStopTimeRequest, ListStopTimesRequest, StopTime, StopTimesReply,
};

/// A stored document paired with its id, flattened onto the wire.
///
/// Replies use this shape so the id and the document fields share one JSON
/// object: `{"id": "stop-001", "stop_name": "...", ...}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentRecord<T> {
    /// Document id (the collection's primary key).
    pub id: String,
    /// The document payload, flattened into the same JSON object.
    #[serde(flatten)]
    pub document: T,
}

/// Deserialize query-result documents into typed records.
fn records<T: DeserializeOwned>(documents: &[Document]) -> anyhow::Result<Vec<DocumentRecord<T>>> {
    documents
        .iter()
        .map(|doc| {
            let document = serde_json::from_slice(&doc.data).context("deserializing document")?;
            Ok(DocumentRecord {
                id: doc.id.clone(),
                document,
            })
        })
        .collect()
}
