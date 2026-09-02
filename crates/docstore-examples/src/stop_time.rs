//! Stop-time collection: create, get, and a combined filter query.
//!
//! The list handler combines `eq` (trip, stop) with `gte`/`lte` ranges over
//! both a string field (arrival time) and a numeric field (stop sequence),
//! sorted by sequence.

use anyhow::Context as _;
use omnia_guest::api::Context;
use omnia_guest::document_store::{Document, Filter, QueryOptions, SortField};
use omnia_guest::{DocumentStore, Result, not_found};
use serde::{Deserialize, Serialize};

use crate::{DocumentRecord, records};

/// Document collection holding the stop times.
pub const COLLECTION: &str = "stop_times";

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

/// Create a stop time with a caller-chosen id (fails when the id exists).
#[derive(Debug, Clone, Deserialize)]
pub struct CreateStopTimeRequest {
    /// Document id for the new stop time.
    pub id: String,
    /// The stop-time record, flattened alongside the id in the JSON body.
    #[serde(flatten)]
    pub stop_time: StopTime,
}

#[omnia_guest::handler]
async fn create_stop_time_request<P>(
    input: CreateStopTimeRequest, context: Context<'_, P>,
) -> Result<DocumentRecord<StopTime>>
where
    P: DocumentStore,
{
    let document = Document {
        id: input.id.clone(),
        data: serde_json::to_vec(&input.stop_time).context("serializing stop time")?,
    };
    DocumentStore::insert(context.provider, COLLECTION, &document).await?;

    Ok(DocumentRecord {
        id: input.id,
        document: input.stop_time,
    })
}

/// Fetch one stop time by id.
#[derive(Debug, Clone, Deserialize)]
pub struct GetStopTimeRequest {
    /// Document id (path parameter).
    pub id: String,
}

#[omnia_guest::handler]
async fn get_stop_time_request<P>(
    input: GetStopTimeRequest, context: Context<'_, P>,
) -> Result<DocumentRecord<StopTime>>
where
    P: DocumentStore,
{
    let document = DocumentStore::get(context.provider, COLLECTION, &input.id)
        .await?
        .ok_or_else(|| not_found!("stop time {} not found", input.id))?;
    let stop_time = serde_json::from_slice(&document.data).context("deserializing stop time")?;

    Ok(DocumentRecord {
        id: document.id,
        document: stop_time,
    })
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
#[derive(Debug, Clone, Serialize)]
pub struct StopTimesReply {
    /// Matches sorted by `stop_sequence` ascending.
    pub stop_times: Vec<DocumentRecord<StopTime>>,
    /// Token for the next page, when more matches remain.
    pub continuation: Option<String>,
}

#[omnia_guest::handler]
async fn list_stop_times_request<P>(
    input: ListStopTimesRequest, context: Context<'_, P>,
) -> Result<StopTimesReply>
where
    P: DocumentStore,
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

    let result = DocumentStore::query(
        context.provider,
        COLLECTION,
        QueryOptions {
            filter,
            order_by: vec![SortField {
                field: "stop_sequence".to_string(),
                descending: false,
            }],
            limit: input.limit,
            continuation: input.continuation,
            ..Default::default()
        },
    )
    .await?;

    Ok(StopTimesReply {
        stop_times: records(&result.documents)?,
        continuation: result.continuation,
    })
}
