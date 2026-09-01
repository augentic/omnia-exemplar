//! Agency handlers: list, create, get, and partial update.
//!
//! Demonstrates server-assigned ids (a max-id probe, then max + 1),
//! existence checks answering 404, [`UpdateBuilder`] with conditional
//! `.set()`s so only provided fields are written, and a fetch-after-update
//! reply so the caller sees exactly what was stored.

use anyhow::Context as _;
use chrono::Utc;
use omnia_guest::api::Context;
use omnia_guest::orm::{Entity as _, Filter, InsertBuilder, SelectBuilder, UpdateBuilder};
use omnia_guest::{Result, TableStore, bad_request, entity, not_found};
use serde::{Deserialize, Serialize};

use crate::{CONNECTION, schema};

entity!(
    table = "agency",
    /// A transit agency.
    #[derive(Debug, Clone, Serialize)]
    pub struct Agency {
        /// Server-assigned primary key.
        pub agency_id: i64,
        /// Display name.
        pub name: String,
        /// Homepage URL.
        pub url: Option<String>,
        /// IANA timezone name.
        pub timezone: Option<String>,
        /// Creation timestamp (`YYYY-MM-DD HH:MM:SS`).
        pub created_at: String,
    }
);

/// Fetch a single agency by id, or `None` when it does not exist.
pub(crate) async fn fetch_agency<P: TableStore>(
    provider: &P, id: i64,
) -> anyhow::Result<Option<Agency>> {
    let query = SelectBuilder::<Agency>::new()
        .r#where(Filter::eq("agency_id", id))
        .build()
        .context("building agency fetch")?;

    let rows = TableStore::query(provider, CONNECTION.to_string(), query.sql, query.params).await?;
    rows.first().map(Agency::from_row).transpose().context("mapping agency row")
}

/// Compute the next server-assigned agency id (max + 1).
///
/// A concurrent create simply fails on the primary key — acceptable for an
/// example; production code would use a database sequence.
async fn next_agency_id<P: TableStore>(provider: &P) -> anyhow::Result<i64> {
    let query = SelectBuilder::<Agency>::new()
        .order_by_desc(None, "agency_id")
        .limit(1)
        .build()
        .context("building max agency id probe")?;

    let rows = TableStore::query(provider, CONNECTION.to_string(), query.sql, query.params).await?;
    let newest = rows.first().map(Agency::from_row).transpose().context("mapping agency row")?;
    Ok(newest.map_or(1, |agency| agency.agency_id + 1))
}

/// List agencies, newest first.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListAgenciesRequest {
    /// Optional cap on the number of rows returned.
    pub limit: Option<u64>,
}

/// Agencies sorted by creation time, newest first.
#[derive(Debug, Clone, Serialize)]
pub struct AgenciesReply {
    /// The matching agencies.
    pub agencies: Vec<Agency>,
}

#[omnia_guest::handler]
async fn list_agencies_request<P>(
    input: ListAgenciesRequest, context: Context<'_, P>,
) -> Result<AgenciesReply>
where
    P: TableStore,
{
    schema::ensure(context.provider).await?;

    let mut select = SelectBuilder::<Agency>::new().order_by_desc(None, "created_at");
    if let Some(limit) = input.limit {
        select = select.limit(limit);
    }
    let query = select.build().context("building agency list")?;

    let rows = TableStore::query(context.provider, CONNECTION.to_string(), query.sql, query.params)
        .await?;
    let agencies = rows
        .iter()
        .map(Agency::from_row)
        .collect::<anyhow::Result<Vec<_>>>()
        .context("mapping agency rows")?;

    Ok(AgenciesReply { agencies })
}

/// Create an agency with a server-assigned id.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateAgencyRequest {
    /// Display name.
    pub name: String,
    /// Homepage URL.
    pub url: Option<String>,
    /// IANA timezone name.
    pub timezone: Option<String>,
}

/// The agency row after the operation.
#[derive(Debug, Clone, Serialize)]
pub struct AgencyReply {
    /// The stored agency.
    pub agency: Agency,
}

#[omnia_guest::handler]
async fn create_agency_request<P>(
    input: CreateAgencyRequest, context: Context<'_, P>,
) -> Result<AgencyReply>
where
    P: TableStore,
{
    schema::ensure(context.provider).await?;

    let agency = Agency {
        agency_id: next_agency_id(context.provider).await?,
        name: input.name,
        url: input.url,
        timezone: input.timezone,
        created_at: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };

    let query = InsertBuilder::from_entity(&agency).build().context("building agency insert")?;
    TableStore::exec(context.provider, CONNECTION.to_string(), query.sql, query.params).await?;

    Ok(AgencyReply { agency })
}

/// Fetch one agency by id.
#[derive(Debug, Clone, Deserialize)]
pub struct GetAgencyRequest {
    /// Agency id (path parameter).
    pub id: i64,
}

#[omnia_guest::handler]
async fn get_agency_request<P>(
    input: GetAgencyRequest, context: Context<'_, P>,
) -> Result<AgencyReply>
where
    P: TableStore,
{
    schema::ensure(context.provider).await?;

    let agency = fetch_agency(context.provider, input.id)
        .await?
        .ok_or_else(|| not_found!("agency {} not found", input.id))?;

    Ok(AgencyReply { agency })
}

/// Partially update an agency: only provided fields are written.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateAgencyRequest {
    /// Agency id (path parameter, delivered as a numeric string).
    #[serde(deserialize_with = "crate::numeric_id")]
    pub id: i64,
    /// Replacement display name.
    pub name: Option<String>,
    /// Replacement homepage URL.
    pub url: Option<String>,
    /// Replacement IANA timezone name.
    pub timezone: Option<String>,
}

#[omnia_guest::handler]
async fn update_agency_request<P>(
    input: UpdateAgencyRequest, context: Context<'_, P>,
) -> Result<AgencyReply>
where
    P: TableStore,
{
    schema::ensure(context.provider).await?;

    if fetch_agency(context.provider, input.id).await?.is_none() {
        return Err(not_found!("agency {} not found", input.id));
    }

    // Conditionally set only the provided fields.
    let mut update = UpdateBuilder::<Agency>::new();
    if let Some(name) = input.name {
        update = update.set("name", name);
    }
    if let Some(url) = input.url {
        update = update.set("url", url);
    }
    if let Some(timezone) = input.timezone {
        update = update.set("timezone", timezone);
    }

    // An empty patch fails the builder's "no SET clause" guard: a 400, not
    // a server error.
    let query = update
        .r#where(Filter::eq("agency_id", input.id))
        .build()
        .map_err(|error| bad_request!("{}", error))?;
    TableStore::exec(context.provider, CONNECTION.to_string(), query.sql, query.params).await?;

    // Fetch after update so the reply reflects exactly what was stored.
    let agency = fetch_agency(context.provider, input.id)
        .await?
        .ok_or_else(|| not_found!("agency {} not found after update", input.id))?;

    Ok(AgencyReply { agency })
}
