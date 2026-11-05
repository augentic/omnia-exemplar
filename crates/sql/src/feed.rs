//! Feed handlers: list per agency, create, list all with a JOIN, delete.
//!
//! Demonstrates multi-column JOIN aliasing — [`FeedWithAgency`] selects
//! three columns from the joined `agency` table — plus a referential check
//! (a feed for a missing agency is rejected) and `DELETE` with 404 on zero
//! rows affected.

use chrono::Utc;
use common::{Capabilities, RowExt as _};
use omnia_sdk::api::http::{delete, get, post};
use omnia_sdk::api::{Client, Context};
use omnia_sdk::sql::{DataType, Row};
use omnia_sdk::{Result, TableStore, not_found};
use serde::{Deserialize, Serialize};

use crate::agency::fetch_agency;
use crate::schema::{self, CONNECTION};

/// Feeds of one agency: list (GET) and create (POST).
pub const AGENCY_FEEDS: &str = "/examples/agencies/{agency_id}/feeds";

/// All feeds joined with their agency details (GET).
pub const FEEDS: &str = "/examples/feeds";

/// One feed: delete (DELETE).
pub const FEED: &str = "/examples/feeds/{id}";

/// Default page size for the joined feed listing.
const DEFAULT_LIMIT: u64 = 100;

pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {
    axum::Router::new()
        .route(AGENCY_FEEDS, get(list_agency_feeds).merge(post(create_feed)))
        .route(FEEDS, get(list_all_feeds))
        .route(FEED, delete(delete_feed))
}

/// A GTFS feed belonging to an agency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl Feed {
    /// Map a SQL [`Row`] into a [`Feed`].
    ///
    /// # Errors
    ///
    /// Returns an error when any required column is missing, null, or has an unexpected type.
    pub fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            feed_id: row.get_i64("feed_id")?,
            agency_id: row.get_i64("agency_id")?,
            description: row.get_str("description")?.to_string(),
            created_at: row.get_str("created_at")?.to_string(),
        })
    }
}

/// A feed row joined with its agency's details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl FeedWithAgency {
    /// Map a joined SQL [`Row`] into a [`FeedWithAgency`].
    ///
    /// # Errors
    ///
    /// Returns an error when any required column is missing, null, or has an unexpected type.
    pub fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            feed_id: row.get_i64("feed_id")?,
            agency_id: row.get_i64("agency_id")?,
            description: row.get_str("description")?.to_string(),
            created_at: row.get_str("created_at")?.to_string(),
            agency_name: row.get_str("agency_name")?.to_string(),
            agency_url: row.get_opt_str("agency_url")?.map(str::to_owned),
            agency_timezone: row.get_opt_str("agency_timezone")?.map(str::to_owned),
        })
    }
}

/// Compute the next server-assigned feed id (max + 1).
async fn next_feed_id<P: TableStore>(provider: &P) -> Result<i64> {
    let rows = TableStore::query(
        provider,
        CONNECTION.to_string(),
        "SELECT feed_id, agency_id, description, created_at FROM feed ORDER BY feed_id DESC LIMIT $1"
            .to_string(),
        vec![DataType::Uint64(Some(1))],
    )
    .await?;
    let newest = rows.first().map(Feed::from_row).transpose()?;
    Ok(newest.map_or(1, |feed| feed.feed_id + 1))
}

/// List the feeds of one agency, newest first.
#[derive(Debug, Clone, Deserialize)]
pub struct ListAgencyFeedsRequest {
    /// Owning agency id (path parameter).
    pub agency_id: i64,
}

/// Feeds sorted by creation time, newest first.
#[derive(Debug, Clone, Serialize)]
pub struct FeedsReply {
    /// The matching feeds.
    pub feeds: Vec<Feed>,
}

/// Lists the feeds of one agency, newest first.
///
/// # Errors
///
/// Returns an error when the schema cannot be ensured, the select fails, or
/// a row cannot be mapped to a [`Feed`].
pub async fn list_agency_feeds<P>(
    input: ListAgencyFeedsRequest, context: Context<P>,
) -> Result<FeedsReply>
where
    P: TableStore,
{
    let provider = context.provider();
    schema::ensure(provider).await?;

    let rows = TableStore::query(
        provider,
        CONNECTION.to_string(),
        "SELECT feed_id, agency_id, description, created_at FROM feed WHERE agency_id = $1 ORDER BY created_at DESC"
            .to_string(),
        vec![DataType::Int64(Some(input.agency_id))],
    )
    .await?;
    let feeds = rows.iter().map(Feed::from_row).collect::<Result<Vec<_>>>()?;

    Ok(FeedsReply { feeds })
}

/// Create a feed for an existing agency, with a server-assigned id.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateFeedRequest {
    /// Owning agency id (path parameter, delivered as a numeric string).
    #[serde(deserialize_with = "common::numeric_id")]
    pub agency_id: i64,
    /// Human description of the feed contents.
    pub description: String,
}

/// The feed row after the operation.
#[derive(Debug, Clone, Serialize)]
pub struct FeedReply {
    /// The stored feed.
    pub feed: Feed,
}

/// Creates a feed for an existing agency, with a server-assigned id (max + 1).
///
/// # Errors
///
/// Returns `not_found` when the owning agency does not exist, or an error
/// when the schema cannot be ensured, the id probe or the insert fails, or
/// a probed row cannot be mapped.
pub async fn create_feed<P>(input: CreateFeedRequest, context: Context<P>) -> Result<FeedReply>
where
    P: TableStore,
{
    let provider = context.provider();
    schema::ensure(provider).await?;

    // Referential check: a feed for a missing agency is a 404, not an
    // orphaned row.
    if fetch_agency(provider, input.agency_id).await?.is_none() {
        return Err(not_found!("agency {} not found", input.agency_id));
    }

    let feed = Feed {
        feed_id: next_feed_id(provider).await?,
        agency_id: input.agency_id,
        description: input.description,
        created_at: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };

    TableStore::exec(
        provider,
        CONNECTION.to_string(),
        "INSERT INTO feed (feed_id, agency_id, description, created_at) VALUES ($1, $2, $3, $4)"
            .to_string(),
        vec![
            DataType::Int64(Some(feed.feed_id)),
            DataType::Int64(Some(feed.agency_id)),
            DataType::Str(Some(feed.description.clone())),
            DataType::Str(Some(feed.created_at.clone())),
        ],
    )
    .await?;

    Ok(FeedReply { feed })
}

/// List all feeds joined with their agency details.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListAllFeedsRequest {
    /// Optional cap on the number of rows returned (default 100).
    pub limit: Option<u64>,
}

/// Joined feed + agency rows, newest feed first.
#[derive(Debug, Clone, Serialize)]
pub struct FeedsWithAgencyReply {
    /// The matching feeds with their agency columns.
    pub feeds: Vec<FeedWithAgency>,
}

/// Lists all feeds joined with their agency details, newest feed first.
///
/// # Errors
///
/// Returns an error when the schema cannot be ensured, the joined select
/// fails, or a row cannot be mapped to a [`FeedWithAgency`].
pub async fn list_all_feeds<P>(
    input: ListAllFeedsRequest, context: Context<P>,
) -> Result<FeedsWithAgencyReply>
where
    P: TableStore,
{
    let provider = context.provider();
    schema::ensure(provider).await?;

    let rows = TableStore::query(
        provider,
        CONNECTION.to_string(),
        "SELECT feed.feed_id, feed.agency_id, feed.description, feed.created_at, \
         agency.name AS agency_name, agency.url AS agency_url, agency.timezone AS agency_timezone \
         FROM feed LEFT JOIN agency ON feed.agency_id = agency.agency_id \
         ORDER BY feed.created_at DESC LIMIT $1"
            .to_string(),
        vec![DataType::Uint64(Some(input.limit.unwrap_or(DEFAULT_LIMIT)))],
    )
    .await?;
    let feeds = rows.iter().map(FeedWithAgency::from_row).collect::<Result<Vec<_>>>()?;

    Ok(FeedsWithAgencyReply { feeds })
}

/// Delete one feed by id.
#[derive(Debug, Clone, Deserialize)]
pub struct DeleteFeedRequest {
    /// Feed id (path parameter).
    pub id: i64,
}

/// Confirmation of a feed deletion.
#[derive(Debug, Clone, Serialize)]
pub struct DeleteFeedReply {
    /// Id of the removed feed.
    pub feed_id: i64,
}

/// Deletes one feed by id.
///
/// # Errors
///
/// Returns `not_found` when the delete affects zero rows, or an error when
/// the schema cannot be ensured or the delete fails.
pub async fn delete_feed<P>(
    input: DeleteFeedRequest, context: Context<P>,
) -> Result<DeleteFeedReply>
where
    P: TableStore,
{
    let provider = context.provider();
    schema::ensure(provider).await?;

    let affected = TableStore::exec(
        provider,
        CONNECTION.to_string(),
        "DELETE FROM feed WHERE feed_id = $1".to_string(),
        vec![DataType::Int64(Some(input.id))],
    )
    .await?;
    if affected == 0 {
        return Err(not_found!("feed {} not found", input.id));
    }

    Ok(DeleteFeedReply { feed_id: input.id })
}
