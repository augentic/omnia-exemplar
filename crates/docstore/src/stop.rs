//! Stop collection: full CRUD plus a combined filter query.
//!
//! The list handler builds `Filter::and(...)` from whichever query
//! parameters are present — `contains` (text search), `eq` (zone), `ne`
//! (zone exclusion), `eq` + `is_not_null` (accessibility), `is_null`
//! (top-level stops), `gte`/`lte` (bounding box), and `on_date` (update
//! date) — sorted by name with limit/continuation pagination.

use anyhow::Context as _;
use axum::Json;
use axum::extract::{Path, Query};
use axum::routing::get;
use common::Capabilities;
use omnia_sdk::api::{Context, http};
use omnia_sdk::docstore::{Document, Filter, QueryOptions, SortField};
use omnia_sdk::{DocumentStore, Result, bad_request, not_found};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub(crate) const STOPS: &str = "/examples/stops";
pub(crate) const STOP: &str = "/examples/stops/{id}";
pub(crate) const COLLECTION: &str = "stops";

pub(crate) fn routes<P: Capabilities>() -> http::Routes<P> {
    http::Routes::new()
        .route(STOPS, get(list_stops).post(create_stop))
        .route(STOP, get(get_stop).put(upsert_stop).delete(delete_stop))
}

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
pub(crate) fn records<T: DeserializeOwned>(
    documents: &[Document],
) -> anyhow::Result<Vec<DocumentRecord<T>>> {
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

/// A GTFS-like stop stored as one JSON document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stop {
    /// Display name.
    pub stop_name: String,
    /// Latitude in degrees.
    pub stop_lat: f64,
    /// Longitude in degrees.
    pub stop_lon: f64,
    /// Fare zone, when the stop belongs to one.
    pub zone_id: Option<String>,
    /// `1` when wheelchair boarding is possible.
    pub wheelchair_boarding: Option<i32>,
    /// GTFS location type (`0` stop/platform, `1` station, ...).
    pub location_type: Option<i32>,
    /// Parent station id for stops inside a station.
    pub parent_station: Option<String>,
    /// ISO-8601 timestamp of the last update.
    pub last_updated: Option<String>,
}

/// Create a stop with a caller-chosen id (fails when the id exists).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateStopRequest {
    /// Document id for the new stop.
    pub id: String,
    /// The stop record, flattened alongside the id in the JSON body.
    #[serde(flatten)]
    pub stop: Stop,
}

pub(crate) async fn create_stop<P>(
    context: Context<P>, Json(input): Json<CreateStopRequest>,
) -> Result<Json<DocumentRecord<Stop>>>
where
    P: DocumentStore + 'static,
{
    let document = Document {
        id: input.id.clone(),
        data: serde_json::to_vec(&input.stop).context("serializing stop")?,
    };
    DocumentStore::insert(context.provider(), COLLECTION, &document).await?;

    Ok(Json(DocumentRecord {
        id: input.id,
        document: input.stop,
    }))
}

pub(crate) async fn get_stop<P>(
    context: Context<P>, Path(id): Path<String>,
) -> Result<Json<DocumentRecord<Stop>>>
where
    P: DocumentStore + 'static,
{
    let document = DocumentStore::get(context.provider(), COLLECTION, &id)
        .await?
        .ok_or_else(|| not_found!("stop {id} not found"))?;
    let stop = serde_json::from_slice(&document.data).context("deserializing stop")?;

    Ok(Json(DocumentRecord {
        id: document.id,
        document: stop,
    }))
}

// Creates or replaces the whole document under the path's id.
pub(crate) async fn upsert_stop<P>(
    context: Context<P>, Path(id): Path<String>, Json(stop): Json<Stop>,
) -> Result<Json<DocumentRecord<Stop>>>
where
    P: DocumentStore + 'static,
{
    let document = Document {
        id: id.clone(),
        data: serde_json::to_vec(&stop).context("serializing stop")?,
    };
    DocumentStore::put(context.provider(), COLLECTION, &document).await?;

    Ok(Json(DocumentRecord { id, document: stop }))
}

/// Confirmation of a stop deletion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteStopReply {
    /// Id of the removed stop.
    pub id: String,
}

pub(crate) async fn delete_stop<P>(
    context: Context<P>, Path(id): Path<String>,
) -> Result<Json<DeleteStopReply>>
where
    P: DocumentStore + 'static,
{
    let removed = DocumentStore::delete(context.provider(), COLLECTION, &id).await?;
    if !removed {
        return Err(not_found!("stop {id} not found"));
    }

    Ok(Json(DeleteStopReply { id }))
}

/// Query stops with any combination of the supported filters.
///
/// Every field is optional; present fields are combined with AND. Results are
/// sorted by `stop_name` ascending.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListStopsRequest {
    /// Text search: `contains` on `stop_name`.
    pub q: Option<String>,
    /// Fare zone: `eq` on `zone_id`.
    pub zone: Option<String>,
    /// Zone exclusion: `ne` on `zone_id` (the direct `Ne` codepath).
    pub exclude_zone: Option<String>,
    /// When `true`: `eq(wheelchair_boarding, 1)` + `is_not_null(zone_id)`.
    pub accessible: Option<bool>,
    /// When `true`: `is_null(parent_station)`.
    pub top_level: Option<bool>,
    /// Bounding box south edge: `gte` on `stop_lat`.
    pub min_lat: Option<f64>,
    /// Bounding box north edge: `lte` on `stop_lat`.
    pub max_lat: Option<f64>,
    /// Bounding box west edge: `gte` on `stop_lon`.
    pub min_lon: Option<f64>,
    /// Bounding box east edge: `lte` on `stop_lon`.
    pub max_lon: Option<f64>,
    /// Calendar date (`YYYY-MM-DD`): `on_date` on `last_updated`.
    pub updated_on: Option<String>,
    /// Maximum documents per page.
    pub limit: Option<u32>,
    /// Continuation token from the previous page.
    pub continuation: Option<String>,
}

/// One page of matching stops.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopsReply {
    /// Matches sorted by `stop_name` ascending.
    pub stops: Vec<DocumentRecord<Stop>>,
    /// Token for the next page, when more matches remain.
    pub continuation: Option<String>,
}

pub(crate) async fn list_stops<P>(
    context: Context<P>, Query(input): Query<ListStopsRequest>,
) -> Result<Json<StopsReply>>
where
    P: DocumentStore + 'static,
{
    let mut filters = Vec::new();

    if let Some(q) = &input.q {
        filters.push(Filter::contains("stop_name", q));
    }
    if let Some(zone) = &input.zone {
        filters.push(Filter::eq("zone_id", zone.as_str()));
    }
    if let Some(zone) = &input.exclude_zone {
        filters.push(Filter::ne("zone_id", zone.as_str()));
    }
    if input.accessible.unwrap_or(false) {
        filters.push(Filter::eq("wheelchair_boarding", 1));
        filters.push(Filter::is_not_null("zone_id"));
    }
    if input.top_level.unwrap_or(false) {
        filters.push(Filter::is_null("parent_station"));
    }
    if let Some(v) = input.min_lat {
        filters.push(Filter::gte("stop_lat", v));
    }
    if let Some(v) = input.max_lat {
        filters.push(Filter::lte("stop_lat", v));
    }
    if let Some(v) = input.min_lon {
        filters.push(Filter::gte("stop_lon", v));
    }
    if let Some(v) = input.max_lon {
        filters.push(Filter::lte("stop_lon", v));
    }
    if let Some(date) = &input.updated_on {
        let filter =
            Filter::on_date("last_updated", date).map_err(|error| bad_request!("{}", error))?;
        filters.push(filter);
    }

    let filter = if filters.is_empty() { None } else { Some(Filter::and(filters)) };

    let result = DocumentStore::query(
        context.provider(),
        COLLECTION,
        QueryOptions {
            filter,
            order_by: vec![SortField {
                field: "stop_name".to_string(),
                descending: false,
            }],
            limit: input.limit,
            continuation: input.continuation,
            ..Default::default()
        },
    )
    .await?;

    Ok(Json(StopsReply {
        stops: records(&result.documents)?,
        continuation: result.continuation,
    }))
}
