//! Feed handlers: list per agency, create, list all with a JOIN, delete.
//!
//! Demonstrates multi-column JOIN aliasing — [`FeedWithAgency`] selects
//! three columns from the joined `agency` table — plus a referential check
//! (a feed for a missing agency is rejected) and `DELETE` with 404 on zero
//! rows affected.

use axum::Json;
use axum::extract::{Path, Query};
use axum::routing::{delete, get};
use chrono::Utc;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::sql::{FromRow, Value};
use omnia_sdk::{Result, SqlDb, SqlDbExt as _, not_found};
use serde::{Deserialize, Serialize};

use crate::agency::fetch_agency;
use crate::schema;

pub(crate) const AGENCY_FEEDS: &str = "/examples/agencies/{agency_id}/feeds";
pub(crate) const FEEDS: &str = "/examples/feeds";
pub(crate) const FEED: &str = "/examples/feeds/{id}";

const FEED_COLUMNS: &str = "feed_id, agency_id, description, created_at";
const DEFAULT_LIMIT: u32 = 100;

pub(crate) fn handlers<P: crate::Requires>() -> Handlers<P> {
    Handlers::new()
        .route(AGENCY_FEEDS, get(list_agency_feeds).post(create_feed))
        .route(FEEDS, get(list_all_feeds))
        .route(FEED, delete(delete_feed))
}

/// A GTFS feed belonging to an agency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Feed {
    /// Server-assigned primary key.
    pub feed_id: i64,
    /// Owning agency id.
    pub agency_id: i64,
    /// Human description of the feed contents.
    pub description: String,
    /// Creation timestamp (`YYYY-MM-DD HH:MM:SS`).
    pub created_at: String,
}

/// A feed row joined with its agency's details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct FeedWithAgency {
    /// Server-assigned primary key (`feed.feed_id`).
    pub feed_id: i64,
    /// Owning agency id (`feed.agency_id`).
    pub agency_id: i64,
    /// Human description (`feed.description`).
    pub description: String,
    /// Creation timestamp (`feed.created_at`).
    pub created_at: String,
    /// Agency display name (`agency.name`).
    pub agency_name: String,
    /// Agency homepage (`agency.url`).
    pub agency_url: Option<String>,
    /// Agency timezone (`agency.timezone`).
    pub agency_timezone: Option<String>,
}

/// Feeds sorted by creation time, newest first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedsReply {
    /// The matching feeds.
    pub feeds: Vec<Feed>,
}

pub(crate) async fn list_agency_feeds<P>(
    context: Context<P>, Path(agency_id): Path<i64>,
) -> Result<Json<FeedsReply>>
where
    P: SqlDb,
{
    let db = context.sql();
    schema::ensure(db).await?;

    let feeds = db
        .query_as(
            &format!(
                "SELECT {FEED_COLUMNS} FROM feed WHERE agency_id = $1 ORDER BY created_at DESC"
            ),
            &[Value::from(agency_id)],
        )
        .await?;

    Ok(Json(FeedsReply { feeds }))
}

/// Create a feed for an existing agency, with a server-assigned id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFeedRequest {
    /// Human description of the feed contents.
    pub description: String,
}

/// The feed row after the operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedReply {
    /// The stored feed.
    pub feed: Feed,
}

pub(crate) async fn create_feed<P>(
    context: Context<P>, Path(agency_id): Path<i64>, Json(input): Json<CreateFeedRequest>,
) -> Result<Json<FeedReply>>
where
    P: SqlDb,
{
    let db = context.sql();
    schema::ensure(db).await?;

    // referential check: a feed for a missing agency is a 404, not an orphaned row
    if fetch_agency(db, agency_id).await?.is_none() {
        return Err(not_found!("agency {agency_id} not found"));
    }

    let feed = Feed {
        feed_id: next_id(db).await?,
        agency_id,
        description: input.description,
        created_at: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };

    db.execute(
        &format!("INSERT INTO feed ({FEED_COLUMNS}) VALUES ($1, $2, $3, $4)"),
        &[
            Value::from(feed.feed_id),
            Value::from(feed.agency_id),
            Value::from(feed.description.as_str()),
            Value::from(feed.created_at.as_str()),
        ],
    )
    .await?;

    Ok(Json(FeedReply { feed }))
}

async fn next_id<P: SqlDb>(db: &P) -> Result<i64> {
    let rows = db.query("SELECT feed_id FROM feed ORDER BY feed_id DESC LIMIT 1", &[]).await?;
    let newest: Option<i64> = rows.first().map(|row| row.get("feed_id")).transpose()?;
    Ok(newest.map_or(1, |id| id + 1))
}

/// List all feeds joined with their agency details.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListAllFeedsRequest {
    /// Optional cap on the number of rows returned (default 100).
    pub limit: Option<u32>,
}

/// Joined feed + agency rows, newest feed first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedsWithAgencyReply {
    /// The matching feeds with their agency columns.
    pub feeds: Vec<FeedWithAgency>,
}

pub(crate) async fn list_all_feeds<P>(
    context: Context<P>, Query(input): Query<ListAllFeedsRequest>,
) -> Result<Json<FeedsWithAgencyReply>>
where
    P: SqlDb,
{
    let db = context.sql();
    schema::ensure(db).await?;

    let feeds = db
        .query_as(
            "SELECT feed.feed_id, feed.agency_id, feed.description, feed.created_at, \
             agency.name AS agency_name, agency.url AS agency_url, agency.timezone AS agency_timezone \
             FROM feed LEFT JOIN agency ON feed.agency_id = agency.agency_id \
             ORDER BY feed.created_at DESC LIMIT $1",
            &[Value::from(i64::from(input.limit.unwrap_or(DEFAULT_LIMIT)))],
        )
        .await?;

    Ok(Json(FeedsWithAgencyReply { feeds }))
}

/// Confirmation of a feed deletion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteFeedReply {
    /// Id of the removed feed.
    pub feed_id: i64,
}

pub(crate) async fn delete_feed<P>(
    context: Context<P>, Path(id): Path<i64>,
) -> Result<Json<DeleteFeedReply>>
where
    P: SqlDb,
{
    let db = context.sql();
    schema::ensure(db).await?;

    let affected = db.execute("DELETE FROM feed WHERE feed_id = $1", &[Value::from(id)]).await?;
    if affected == 0 {
        return Err(not_found!("feed {id} not found"));
    }

    Ok(Json(DeleteFeedReply { feed_id: id }))
}
