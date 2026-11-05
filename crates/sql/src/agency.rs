//! Agency handlers: list, create, get, and partial update.
//!
//! Demonstrates server-assigned ids (a max-id probe, then max + 1),
//! existence checks answering 404, conditional `SET` clauses so only
//! provided fields are written, and a fetch-after-update reply so the
//! caller sees exactly what was stored.

use chrono::Utc;
use common::{Capabilities, RowExt as _};
use omnia_sdk::api::http::{get, patch, post};
use omnia_sdk::api::{Client, Context};
use omnia_sdk::sql::{DataType, Row};
use omnia_sdk::{Result, TableStore, bad_request, not_found};
use serde::{Deserialize, Serialize};

use crate::schema::{self, CONNECTION};

/// Agency collection: list (GET) and create (POST).
pub const AGENCIES: &str = "/examples/agencies";

/// One agency: get (GET) and partial update (PATCH).
pub const AGENCY: &str = "/examples/agencies/{id}";

pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {
    axum::Router::new()
        .route(AGENCIES, get(list_agencies).merge(post(create_agency)))
        .route(AGENCY, get(get_agency).merge(patch(update_agency)))
}

/// A transit agency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl Agency {
    /// Map a SQL [`Row`] into an [`Agency`].
    ///
    /// # Errors
    ///
    /// Returns an error when any required column is missing, null, or has an unexpected type.
    pub fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            agency_id: row.get_i64("agency_id")?,
            name: row.get_str("name")?.to_string(),
            url: row.get_opt_str("url")?.map(str::to_owned),
            timezone: row.get_opt_str("timezone")?.map(str::to_owned),
            created_at: row.get_str("created_at")?.to_string(),
        })
    }
}

/// Fetch a single agency by id, or `None` when it does not exist.
pub(crate) async fn fetch_agency<P: TableStore>(provider: &P, id: i64) -> Result<Option<Agency>> {
    let rows = TableStore::query(
        provider,
        CONNECTION.to_string(),
        "SELECT agency_id, name, url, timezone, created_at FROM agency WHERE agency_id = $1"
            .to_string(),
        vec![DataType::Int64(Some(id))],
    )
    .await?;
    rows.first().map(Agency::from_row).transpose()
}

/// Compute the next server-assigned agency id (max + 1).
///
/// A concurrent create simply fails on the primary key — acceptable for an
/// example; production code would use a database sequence.
async fn next_agency_id<P: TableStore>(provider: &P) -> Result<i64> {
    let rows = TableStore::query(
        provider,
        CONNECTION.to_string(),
        "SELECT agency_id, name, url, timezone, created_at FROM agency ORDER BY agency_id DESC LIMIT $1"
            .to_string(),
        vec![DataType::Uint64(Some(1))],
    )
    .await?;
    let newest = rows.first().map(Agency::from_row).transpose()?;
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

/// Lists agencies, newest first, with an optional row cap.
///
/// # Errors
///
/// Returns an error when the schema cannot be ensured, the select fails, or
/// a row cannot be mapped to an [`Agency`].
pub async fn list_agencies<P>(
    input: ListAgenciesRequest, context: Context<P>,
) -> Result<AgenciesReply>
where
    P: TableStore,
{
    let provider = context.provider();
    schema::ensure(provider).await?;

    let (sql, params) = input.limit.map_or_else(
        || {
            (
                "SELECT agency_id, name, url, timezone, created_at FROM agency ORDER BY created_at DESC"
                    .to_string(),
                vec![],
            )
        },
        |limit| {
            (
                "SELECT agency_id, name, url, timezone, created_at FROM agency ORDER BY created_at DESC LIMIT $1"
                    .to_string(),
                vec![DataType::Uint64(Some(limit))],
            )
        },
    );

    let rows = TableStore::query(provider, CONNECTION.to_string(), sql, params).await?;
    let agencies = rows.iter().map(Agency::from_row).collect::<Result<Vec<_>>>()?;

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

/// Creates an agency with a server-assigned id (max + 1).
///
/// # Errors
///
/// Returns an error when the schema cannot be ensured, the id probe or the
/// insert fails, or a probed row cannot be mapped.
pub async fn create_agency<P>(
    input: CreateAgencyRequest, context: Context<P>,
) -> Result<AgencyReply>
where
    P: TableStore,
{
    let provider = context.provider();
    schema::ensure(provider).await?;

    let agency = Agency {
        agency_id: next_agency_id(provider).await?,
        name: input.name,
        url: input.url,
        timezone: input.timezone,
        created_at: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };

    TableStore::exec(
        provider,
        CONNECTION.to_string(),
        "INSERT INTO agency (agency_id, name, url, timezone, created_at) VALUES ($1, $2, $3, $4, $5)"
            .to_string(),
        vec![
            DataType::Int64(Some(agency.agency_id)),
            DataType::Str(Some(agency.name.clone())),
            DataType::Str(agency.url.clone()),
            DataType::Str(agency.timezone.clone()),
            DataType::Str(Some(agency.created_at.clone())),
        ],
    )
    .await?;

    Ok(AgencyReply { agency })
}

/// Fetch one agency by id.
#[derive(Debug, Clone, Deserialize)]
pub struct GetAgencyRequest {
    /// Agency id (path parameter).
    pub id: i64,
}

/// Fetches one agency by id.
///
/// # Errors
///
/// Returns `not_found` when no agency has the id, or an error when the
/// schema cannot be ensured or the fetch fails.
pub async fn get_agency<P>(input: GetAgencyRequest, context: Context<P>) -> Result<AgencyReply>
where
    P: TableStore,
{
    let provider = context.provider();
    schema::ensure(provider).await?;

    let agency = fetch_agency(provider, input.id)
        .await?
        .ok_or_else(|| not_found!("agency {} not found", input.id))?;

    Ok(AgencyReply { agency })
}

/// Partially update an agency: only provided fields are written.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateAgencyRequest {
    /// Agency id (path parameter, delivered as a numeric string).
    #[serde(deserialize_with = "common::numeric_id")]
    pub id: i64,
    /// Replacement display name.
    pub name: Option<String>,
    /// Replacement homepage URL.
    pub url: Option<String>,
    /// Replacement IANA timezone name.
    pub timezone: Option<String>,
}

/// Partially updates an agency, writing only the provided fields, and
/// replies with the row as stored after the update.
///
/// # Errors
///
/// Returns `not_found` when no agency has the id, `bad_request` when the
/// patch sets no fields, or an error when the schema cannot be ensured or a
/// statement fails.
pub async fn update_agency<P>(
    input: UpdateAgencyRequest, context: Context<P>,
) -> Result<AgencyReply>
where
    P: TableStore,
{
    let provider = context.provider();
    schema::ensure(provider).await?;

    if fetch_agency(provider, input.id).await?.is_none() {
        return Err(not_found!("agency {} not found", input.id));
    }

    // Conditionally set only the provided fields.
    let mut assignments = Vec::new();
    let mut params = Vec::new();
    for (column, value) in [("name", input.name), ("url", input.url), ("timezone", input.timezone)]
    {
        if let Some(value) = value {
            params.push(DataType::Str(Some(value)));
            assignments.push(format!("{column} = ${}", params.len()));
        }
    }
    if assignments.is_empty() {
        return Err(bad_request!("update sets no columns"));
    }

    params.push(DataType::Int64(Some(input.id)));
    let sql =
        format!("UPDATE agency SET {} WHERE agency_id = ${}", assignments.join(", "), params.len());
    TableStore::exec(provider, CONNECTION.to_string(), sql, params).await?;

    // Fetch after update so the reply reflects exactly what was stored.
    let agency = fetch_agency(provider, input.id)
        .await?
        .ok_or_else(|| not_found!("agency {} not found after update", input.id))?;

    Ok(AgencyReply { agency })
}
