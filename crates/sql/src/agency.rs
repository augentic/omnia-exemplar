//! Agency handlers: list, create, get, and partial update.
//!
//! Demonstrates server-assigned ids (a max-id probe, then max + 1),
//! existence checks answering 404, conditional `SET` clauses so only
//! provided fields are written, and a fetch-after-update reply so the
//! caller sees exactly what was stored.

use axum::Json;
use axum::extract::{Path, Query};
use axum::routing::get;
use chrono::Utc;
use common::{Capabilities, RowExt as _};
use omnia_sdk::api::{Context, http};
use omnia_sdk::sql::{DataType, Row};
use omnia_sdk::{Result, TableStore, bad_request, not_found};
use serde::{Deserialize, Serialize};

use crate::schema::{self, CONNECTION};

pub(crate) const AGENCIES: &str = "/examples/agencies";
pub(crate) const AGENCY: &str = "/examples/agencies/{id}";

const AGENCY_COLUMNS: &str = "agency_id, name, url, timezone, created_at";

pub(crate) fn routes<P: Capabilities>() -> http::Routes<P> {
    http::Routes::new()
        .route(AGENCIES, get(list_agencies).post(create_agency))
        .route(AGENCY, get(get_agency).patch(update_agency))
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

impl TryFrom<&Row> for Agency {
    type Error = omnia_sdk::Error;

    fn try_from(row: &Row) -> Result<Self> {
        Ok(Self {
            agency_id: row.get_i64("agency_id")?,
            name: row.get_str("name")?.to_owned(),
            url: row.get_opt_str("url")?.map(str::to_owned),
            timezone: row.get_opt_str("timezone")?.map(str::to_owned),
            created_at: row.get_str("created_at")?.to_owned(),
        })
    }
}

pub(crate) async fn fetch_agency<P: TableStore>(provider: &P, id: i64) -> Result<Option<Agency>> {
    let rows = TableStore::query(
        provider,
        CONNECTION.to_owned(),
        format!("SELECT {AGENCY_COLUMNS} FROM agency WHERE agency_id = $1"),
        vec![DataType::Int64(Some(id))],
    )
    .await?;
    rows.first().map(Agency::try_from).transpose()
}

// A concurrent create simply fails on the primary key — acceptable for an
// example; production code would use a database sequence.
async fn next_agency_id<P: TableStore>(provider: &P) -> Result<i64> {
    let rows = TableStore::query(
        provider,
        CONNECTION.to_owned(),
        "SELECT agency_id FROM agency ORDER BY agency_id DESC LIMIT 1".to_owned(),
        vec![],
    )
    .await?;
    let newest = rows.first().map(|row| row.get_i64("agency_id")).transpose()?;
    Ok(newest.map_or(1, |id| id + 1))
}

/// List agencies, newest first.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListAgenciesRequest {
    /// Optional cap on the number of rows returned.
    pub limit: Option<u64>,
}

/// Agencies sorted by creation time, newest first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenciesReply {
    /// The matching agencies.
    pub agencies: Vec<Agency>,
}

pub(crate) async fn list_agencies<P>(
    context: Context<P>, Query(input): Query<ListAgenciesRequest>,
) -> Result<Json<AgenciesReply>>
where
    P: TableStore + 'static,
{
    let provider = context.provider();
    schema::ensure(provider).await?;

    let mut sql = format!("SELECT {AGENCY_COLUMNS} FROM agency ORDER BY created_at DESC");
    let mut params = Vec::new();
    if let Some(limit) = input.limit {
        sql.push_str(" LIMIT $1");
        params.push(DataType::Uint64(Some(limit)));
    }

    let rows = TableStore::query(provider, CONNECTION.to_owned(), sql, params).await?;
    let agencies = rows.iter().map(Agency::try_from).collect::<Result<Vec<_>>>()?;

    Ok(Json(AgenciesReply { agencies }))
}

/// Create an agency with a server-assigned id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgencyRequest {
    /// Display name.
    pub name: String,
    /// Homepage URL.
    pub url: Option<String>,
    /// IANA timezone name.
    pub timezone: Option<String>,
}

/// The agency row after the operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgencyReply {
    /// The stored agency.
    pub agency: Agency,
}

pub(crate) async fn create_agency<P>(
    context: Context<P>, Json(input): Json<CreateAgencyRequest>,
) -> Result<Json<AgencyReply>>
where
    P: TableStore + 'static,
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
        CONNECTION.to_owned(),
        format!("INSERT INTO agency ({AGENCY_COLUMNS}) VALUES ($1, $2, $3, $4, $5)"),
        vec![
            DataType::Int64(Some(agency.agency_id)),
            DataType::Str(Some(agency.name.clone())),
            DataType::Str(agency.url.clone()),
            DataType::Str(agency.timezone.clone()),
            DataType::Str(Some(agency.created_at.clone())),
        ],
    )
    .await?;

    Ok(Json(AgencyReply { agency }))
}

pub(crate) async fn get_agency<P>(
    context: Context<P>, Path(id): Path<i64>,
) -> Result<Json<AgencyReply>>
where
    P: TableStore + 'static,
{
    let provider = context.provider();
    schema::ensure(provider).await?;

    let agency =
        fetch_agency(provider, id).await?.ok_or_else(|| not_found!("agency {id} not found"))?;

    Ok(Json(AgencyReply { agency }))
}

/// Partially update an agency: only provided fields are written.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAgencyRequest {
    /// Replacement display name.
    pub name: Option<String>,
    /// Replacement homepage URL.
    pub url: Option<String>,
    /// Replacement IANA timezone name.
    pub timezone: Option<String>,
}

pub(crate) async fn update_agency<P>(
    context: Context<P>, Path(id): Path<i64>, Json(input): Json<UpdateAgencyRequest>,
) -> Result<Json<AgencyReply>>
where
    P: TableStore + 'static,
{
    let provider = context.provider();
    schema::ensure(provider).await?;

    if fetch_agency(provider, id).await?.is_none() {
        return Err(not_found!("agency {id} not found"));
    }

    // conditionally set only the provided fields
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

    params.push(DataType::Int64(Some(id)));
    let sql =
        format!("UPDATE agency SET {} WHERE agency_id = ${}", assignments.join(", "), params.len());
    TableStore::exec(provider, CONNECTION.to_owned(), sql, params).await?;

    // fetch after update so the reply reflects what was stored
    let agency = fetch_agency(provider, id)
        .await?
        .ok_or_else(|| not_found!("agency {id} not found after update"))?;

    Ok(Json(AgencyReply { agency }))
}
