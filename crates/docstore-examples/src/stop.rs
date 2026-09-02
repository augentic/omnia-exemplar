//! Stop collection: full CRUD plus a combined filter query.
//!
//! The list handler builds `Filter::and(...)` from whichever query
//! parameters are present — `contains` (text search), `eq` (zone), `ne`
//! (zone exclusion), `eq` + `is_not_null` (accessibility), `is_null`
//! (top-level stops), `gte`/`lte` (bounding box), and `on_date` (update
//! date) — sorted by name with limit/continuation pagination.

use anyhow::Context as _;
use omnia_guest::api::Context;
use omnia_guest::document_store::{Document, Filter, QueryOptions, SortField};
use omnia_guest::{DocumentStore, Result, bad_request, not_found};
use serde::{Deserialize, Serialize};

use crate::{DocumentRecord, records};

/// Document collection holding the stops.
pub const COLLECTION: &str = "stops";

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
#[derive(Debug, Clone, Deserialize)]
pub struct CreateStopRequest {
    /// Document id for the new stop.
    pub id: String,
    /// The stop record, flattened alongside the id in the JSON body.
    #[serde(flatten)]
    pub stop: Stop,
}

#[omnia_guest::handler]
async fn create_stop_request<P>(
    input: CreateStopRequest, context: Context<'_, P>,
) -> Result<DocumentRecord<Stop>>
where
    P: DocumentStore,
{
    let document = Document {
        id: input.id.clone(),
        data: serde_json::to_vec(&input.stop).context("serializing stop")?,
    };
    DocumentStore::insert(context.provider, COLLECTION, &document).await?;

    Ok(DocumentRecord {
        id: input.id,
        document: input.stop,
    })
}

/// Fetch one stop by id.
#[derive(Debug, Clone, Deserialize)]
pub struct GetStopRequest {
    /// Document id (path parameter).
    pub id: String,
}

#[omnia_guest::handler]
async fn get_stop_request<P>(
    input: GetStopRequest, context: Context<'_, P>,
) -> Result<DocumentRecord<Stop>>
where
    P: DocumentStore,
{
    let document = DocumentStore::get(context.provider, COLLECTION, &input.id)
        .await?
        .ok_or_else(|| not_found!("stop {} not found", input.id))?;
    let stop = serde_json::from_slice(&document.data).context("deserializing stop")?;

    Ok(DocumentRecord {
        id: document.id,
        document: stop,
    })
}

/// Upsert one stop by id (creates or replaces the whole document).
#[derive(Debug, Clone, Deserialize)]
pub struct UpsertStopRequest {
    /// Document id (path parameter).
    pub id: String,
    /// The replacement stop record from the JSON body.
    #[serde(flatten)]
    pub stop: Stop,
}

#[omnia_guest::handler]
async fn upsert_stop_request<P>(
    input: UpsertStopRequest, context: Context<'_, P>,
) -> Result<DocumentRecord<Stop>>
where
    P: DocumentStore,
{
    let document = Document {
        id: input.id.clone(),
        data: serde_json::to_vec(&input.stop).context("serializing stop")?,
    };
    DocumentStore::put(context.provider, COLLECTION, &document).await?;

    Ok(DocumentRecord {
        id: input.id,
        document: input.stop,
    })
}

/// Delete one stop by id.
#[derive(Debug, Clone, Deserialize)]
pub struct DeleteStopRequest {
    /// Document id (path parameter).
    pub id: String,
}

/// Confirmation of a stop deletion.
#[derive(Debug, Clone, Serialize)]
pub struct DeleteStopReply {
    /// Id of the removed stop.
    pub id: String,
}

#[omnia_guest::handler]
async fn delete_stop_request<P>(
    input: DeleteStopRequest, context: Context<'_, P>,
) -> Result<DeleteStopReply>
where
    P: DocumentStore,
{
    let removed = DocumentStore::delete(context.provider, COLLECTION, &input.id).await?;
    if !removed {
        return Err(not_found!("stop {} not found", input.id));
    }

    Ok(DeleteStopReply { id: input.id })
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
#[derive(Debug, Clone, Serialize)]
pub struct StopsReply {
    /// Matches sorted by `stop_name` ascending.
    pub stops: Vec<DocumentRecord<Stop>>,
    /// Token for the next page, when more matches remain.
    pub continuation: Option<String>,
}

#[omnia_guest::handler]
async fn list_stops_request<P>(
    input: ListStopsRequest, context: Context<'_, P>,
) -> Result<StopsReply>
where
    P: DocumentStore,
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
        context.provider,
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

    Ok(StopsReply {
        stops: records(&result.documents)?,
        continuation: result.continuation,
    })
}
