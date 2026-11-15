//! Stop-time collection: create, get, and a combined filter query.
//!
//! The list handler combines `eq` (trip, stop) with `gte`/`lte` ranges over
//! both a string field (arrival time) and a numeric field (stop sequence),
//! sorted by sequence.

use axum::Json;
use axum::extract::{Path, Query};
use axum::routing::get;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::docstore::{Document, Exists, Filter, SortField};
use omnia_sdk::{DocumentDb, Result, not_found};
use serde::{Deserialize, Serialize};

use crate::stop::{CreateError, DocumentRecord, records};

pub(crate) const STOP_TIMES: &str = "/examples/stop-times";
pub(crate) const STOP_TIME: &str = "/examples/stop-times/{id}";
pub(crate) const COLLECTION: &str = "stop_times";

pub(crate) fn handlers<P: crate::Requires>() -> Handlers<P> {
    Handlers::new()
        .route(STOP_TIMES, get(list_stop_times).post(create_stop_time))
        .route(STOP_TIME, get(get_stop_time))
}

/// A GTFS-like stop time stored as one JSON document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopTime {
    /// Trip this stop time belongs to.
    pub trip_id: String,
    /// Stop being served.
    pub stop_id: String,
    /// Arrival time as `HH:MM:SS`.
    pub arrival_time: String,
    /// Departure time as `HH:MM:SS`.
    pub departure_time: String,
    /// Order of the stop within the trip.
    pub stop_sequence: i32,
    /// GTFS pickup type (`0` regular, `1` none, ...).
    pub pickup_type: Option<i32>,
    /// GTFS drop-off type (`0` regular, `1` none, ...).
    pub drop_off_type: Option<i32>,
}

/// Create a stop time with a caller-chosen id (`409 Conflict` when the id exists).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateStopTimeRequest {
    /// Document id for the new stop time.
    pub id: String,
    /// The stop-time record, flattened alongside the id in the JSON body.
    #[serde(flatten)]
    pub stop_time: StopTime,
}

pub(crate) async fn create_stop_time<P>(
    context: Context<P>, Json(input): Json<CreateStopTimeRequest>,
) -> std::result::Result<Json<DocumentRecord<StopTime>>, CreateError>
where
    P: DocumentDb,
{
    let document = Document::json(&input.id, &input.stop_time)?;
    if context.docs().insert(COLLECTION, &document).await? == Err(Exists) {
        return Err(CreateError::Exists { id: input.id });
    }

    Ok(Json(DocumentRecord {
        id: input.id,
        document: input.stop_time,
    }))
}

pub(crate) async fn get_stop_time<P>(
    context: Context<P>, Path(id): Path<String>,
) -> Result<Json<DocumentRecord<StopTime>>>
where
    P: DocumentDb,
{
    let document = context
        .docs()
        .get(COLLECTION, &id)
        .await?
        .ok_or_else(|| not_found!("stop time {id} not found"))?;
    let stop_time = document.decode()?;

    Ok(Json(DocumentRecord {
        id: document.id,
        document: stop_time,
    }))
}

/// Query stop times with any combination of the supported filters.
///
/// Every field is optional; present fields are combined with AND. Results are
/// sorted by `stop_sequence` ascending.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListStopTimesRequest {
    /// Trip filter: `eq` on `trip_id`.
    pub trip: Option<String>,
    /// Stop filter: `eq` on `stop_id`.
    pub stop: Option<String>,
    /// Earliest arrival: `gte` on `arrival_time`.
    pub after: Option<String>,
    /// Latest arrival: `lte` on `arrival_time`.
    pub before: Option<String>,
    /// Lowest sequence number: `gte` on `stop_sequence`.
    pub min_seq: Option<i32>,
    /// Highest sequence number: `lte` on `stop_sequence`.
    pub max_seq: Option<i32>,
    /// Maximum documents per page.
    pub limit: Option<u32>,
    /// Continuation token from the previous page.
    pub continuation: Option<String>,
}

/// One page of matching stop times.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopTimesReply {
    /// Matches sorted by `stop_sequence` ascending.
    pub stop_times: Vec<DocumentRecord<StopTime>>,
    /// Token for the next page, when more matches remain.
    pub continuation: Option<String>,
}

pub(crate) async fn list_stop_times<P>(
    context: Context<P>, Query(input): Query<ListStopTimesRequest>,
) -> Result<Json<StopTimesReply>>
where
    P: DocumentDb,
{
    let mut filters = Vec::new();

    if let Some(trip) = &input.trip {
        filters.push(Filter::eq("trip_id", trip.as_str()));
    }
    if let Some(stop) = &input.stop {
        filters.push(Filter::eq("stop_id", stop.as_str()));
    }
    if let Some(after) = &input.after {
        filters.push(Filter::gte("arrival_time", after.as_str()));
    }
    if let Some(before) = &input.before {
        filters.push(Filter::lte("arrival_time", before.as_str()));
    }
    if let Some(v) = input.min_seq {
        filters.push(Filter::gte("stop_sequence", v));
    }
    if let Some(v) = input.max_seq {
        filters.push(Filter::lte("stop_sequence", v));
    }

    let filter = if filters.is_empty() { None } else { Some(Filter::and(filters)) };

    let query = omnia_sdk::docstore::Query {
        filter,
        order_by: vec![SortField {
            field: "stop_sequence".to_string(),
            descending: false,
        }],
        limit: input.limit,
        after: input.continuation,
        ..Default::default()
    };
    let page = context.docs().query(COLLECTION, &query).await?;

    Ok(Json(StopTimesReply {
        stop_times: records(&page.items)?,
        continuation: page.next,
    }))
}
