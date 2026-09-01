//! Feed handlers: list per agency, create, list all with a JOIN, delete.
//!
//! Demonstrates the `entity!` JOIN support — [`FeedWithAgency`] aliases
//! three columns from the joined `agency` table — plus a referential check
//! (a feed for a missing agency is rejected) and [`DeleteBuilder`] with 404
//! on zero rows affected.

use anyhow::Context as _;
use chrono::Utc;
use omnia_guest::api::Context;
use omnia_guest::orm::{DeleteBuilder, Entity as _, Filter, InsertBuilder, Join, SelectBuilder};
use omnia_guest::{Result, TableStore, entity, not_found};
use serde::{Deserialize, Serialize};

use crate::agency::fetch_agency;
use crate::{CONNECTION, schema};

/// Default page size for the joined feed listing.
const DEFAULT_LIMIT: u64 = 100;

entity!(
    table = "feed",
    /// A GTFS feed belonging to an agency.
    #[derive(Debug, Clone, Serialize)]
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
);

// The `columns` entries list the fields sourced from the joined agency
// table (rendered as `"agency"."name" AS "agency_name"`, ...); fields not
// listed are auto-qualified with the main feed table.
entity!(
    table = "feed",
    columns = [
        ("agency", "name", "agency_name"),
        ("agency", "url", "agency_url"),
        ("agency", "timezone", "agency_timezone"),
    ],
    joins = [Join::left("agency", Filter::col_eq("feed", "agency_id", "agency", "agency_id"))],
    /// A feed row joined with its agency's details.
    #[derive(Debug, Clone, Serialize)]
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
);

/// Compute the next server-assigned feed id (max + 1).
async fn next_feed_id<P: TableStore>(provider: &P) -> anyhow::Result<i64> {
    let query = SelectBuilder::<Feed>::new()
        .order_by_desc(None, "feed_id")
        .limit(1)
        .build()
        .context("building max feed id probe")?;

    let rows = TableStore::query(provider, CONNECTION.to_string(), query.sql, query.params).await?;
    let newest = rows.first().map(Feed::from_row).transpose().context("mapping feed row")?;
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

#[omnia_guest::handler]
async fn list_agency_feeds_request<P>(
    input: ListAgencyFeedsRequest, context: Context<'_, P>,
) -> Result<FeedsReply>
where
    P: TableStore,
{
    schema::ensure(context.provider).await?;

    let query = SelectBuilder::<Feed>::new()
        .r#where(Filter::eq("agency_id", input.agency_id))
        .order_by_desc(None, "created_at")
        .build()
        .context("building agency feed list")?;

    let rows = TableStore::query(context.provider, CONNECTION.to_string(), query.sql, query.params)
        .await?;
    let feeds = rows
        .iter()
        .map(Feed::from_row)
        .collect::<anyhow::Result<Vec<_>>>()
        .context("mapping feed rows")?;

    Ok(FeedsReply { feeds })
}

/// Create a feed for an existing agency, with a server-assigned id.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateFeedRequest {
    /// Owning agency id (path parameter, delivered as a numeric string).
    #[serde(deserialize_with = "crate::numeric_id")]
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

#[omnia_guest::handler]
async fn create_feed_request<P>(
    input: CreateFeedRequest, context: Context<'_, P>,
) -> Result<FeedReply>
where
    P: TableStore,
{
    schema::ensure(context.provider).await?;

    // Referential check: a feed for a missing agency is a 404, not an
    // orphaned row.
    if fetch_agency(context.provider, input.agency_id).await?.is_none() {
        return Err(not_found!("agency {} not found", input.agency_id));
    }

    let feed = Feed {
        feed_id: next_feed_id(context.provider).await?,
        agency_id: input.agency_id,
        description: input.description,
        created_at: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };

    let query = InsertBuilder::from_entity(&feed).build().context("building feed insert")?;
    TableStore::exec(context.provider, CONNECTION.to_string(), query.sql, query.params).await?;

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

#[omnia_guest::handler]
async fn list_all_feeds_request<P>(
    input: ListAllFeedsRequest, context: Context<'_, P>,
) -> Result<FeedsWithAgencyReply>
where
    P: TableStore,
{
    schema::ensure(context.provider).await?;

    let query = SelectBuilder::<FeedWithAgency>::new()
        .order_by_desc(Some("feed"), "created_at")
        .limit(input.limit.unwrap_or(DEFAULT_LIMIT))
        .build()
        .context("building joined feed list")?;

    let rows = TableStore::query(context.provider, CONNECTION.to_string(), query.sql, query.params)
        .await?;
    let feeds = rows
        .iter()
        .map(FeedWithAgency::from_row)
        .collect::<anyhow::Result<Vec<_>>>()
        .context("mapping joined feed rows")?;

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

#[omnia_guest::handler]
async fn delete_feed_request<P>(
    input: DeleteFeedRequest, context: Context<'_, P>,
) -> Result<DeleteFeedReply>
where
    P: TableStore,
{
    schema::ensure(context.provider).await?;

    let query = DeleteBuilder::<Feed>::new()
        .r#where(Filter::eq("feed_id", input.id))
        .build()
        .context("building feed delete")?;

    let affected =
        TableStore::exec(context.provider, CONNECTION.to_string(), query.sql, query.params).await?;
    if affected == 0 {
        return Err(not_found!("feed {} not found", input.id));
    }

    Ok(DeleteFeedReply { feed_id: input.id })
}
