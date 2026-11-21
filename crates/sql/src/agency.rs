//! Agency handlers: list, create, get, and partial update.
//!
//! Demonstrates server-assigned ids (a max-id probe, then max + 1),
//! existence checks answering 404, conditional `SET` clauses so only
//! provided fields are written, and an update that checks, writes, and
//! fetches again in one transaction, so the caller sees exactly what was
//! stored.

use axum::Json;
use axum::extract::{Path, Query};
use axum::routing::get;
use chrono::Utc;
use omnia_sdk::sql::{self, Executor, FromRow, Value};
use omnia_sdk::{Handlers, Result, bad_request, not_found};
use serde::{Deserialize, Serialize};

use crate::schema;

pub(crate) const AGENCIES: &str = "/examples/agencies";
pub(crate) const AGENCY: &str = "/examples/agencies/{id}";

const AGENCY_COLUMNS: &str = "agency_id, name, url, timezone, created_at";

pub(crate) fn handlers() -> Handlers {
    Handlers::new()
        .route(AGENCIES, get(list_agencies).post(create_agency))
        .route(AGENCY, get(get_agency).patch(update_agency))
}

/// A transit agency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
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

pub(crate) async fn fetch_agency(db: &impl Executor, id: i64) -> Result<Option<Agency>> {
    let select = format!("SELECT {AGENCY_COLUMNS} FROM agency WHERE agency_id = $1");
    db.query_opt(&select, sql::params![id]).await
}

/// List agencies, newest first.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListAgenciesRequest {
    /// Optional cap on the number of rows returned.
    pub limit: Option<u32>,
}

/// Agencies sorted by creation time, newest first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenciesReply {
    /// The matching agencies.
    pub agencies: Vec<Agency>,
}

pub(crate) async fn list_agencies(
    Query(input): Query<ListAgenciesRequest>,
) -> Result<Json<AgenciesReply>> {
    let db = schema::open().await?;

    let mut sql = format!("SELECT {AGENCY_COLUMNS} FROM agency ORDER BY created_at DESC");
    let mut params = Vec::new();
    if let Some(limit) = input.limit {
        sql.push_str(" LIMIT $1");
        params.push(Value::from(i64::from(limit)));
    }

    let agencies = db.query_as(&sql, &params).await?;

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

pub(crate) async fn create_agency(
    Json(input): Json<CreateAgencyRequest>,
) -> Result<Json<AgencyReply>> {
    let db = schema::open().await?;

    let agency = Agency {
        agency_id: next_id(&db).await?,
        name: input.name,
        url: input.url,
        timezone: input.timezone,
        created_at: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };

    db.exec(
        &format!("INSERT INTO agency ({AGENCY_COLUMNS}) VALUES ($1, $2, $3, $4, $5)"),
        sql::params![
            agency.agency_id,
            agency.name.as_str(),
            agency.url.as_deref(),
            agency.timezone.as_deref(),
            agency.created_at.as_str(),
        ],
    )
    .await?;

    Ok(Json(AgencyReply { agency }))
}

// A concurrent create simply fails on the primary key — acceptable for an
// example; production code would use a database sequence.
async fn next_id(db: &impl Executor) -> Result<i64> {
    let newest: Option<(i64,)> =
        db.query_opt("SELECT agency_id FROM agency ORDER BY agency_id DESC LIMIT 1", &[]).await?;
    Ok(newest.map_or(1, |(id,)| id + 1))
}

pub(crate) async fn get_agency(Path(id): Path<i64>) -> Result<Json<AgencyReply>> {
    let db = schema::open().await?;

    let agency = fetch_agency(&db, id).await?.ok_or_else(|| not_found!("agency {id} not found"))?;

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

pub(crate) async fn update_agency(
    Path(id): Path<i64>, Json(input): Json<UpdateAgencyRequest>,
) -> Result<Json<AgencyReply>> {
    let db = schema::open().await?;

    let agency = db
        .transaction(async |tx| {
            if fetch_agency(tx, id).await?.is_none() {
                return Err(not_found!("agency {id} not found"));
            }

            // conditionally set only the provided fields
            let mut assignments = Vec::new();
            let mut params = Vec::new();
            let fields = [("name", input.name), ("url", input.url), ("timezone", input.timezone)];
            for (column, value) in fields {
                if let Some(value) = value {
                    params.push(Value::from(value));
                    assignments.push(format!("{column} = ${}", params.len()));
                }
            }
            if assignments.is_empty() {
                return Err(bad_request!("update sets no columns"));
            }

            params.push(Value::from(id));
            let update = format!(
                "UPDATE agency SET {} WHERE agency_id = ${}",
                assignments.join(", "),
                params.len()
            );
            tx.exec(&update, &params).await?;

            // fetch after update so the reply reflects what was stored
            fetch_agency(tx, id)
                .await?
                .ok_or_else(|| not_found!("agency {id} not found after update"))
        })
        .await?;

    Ok(Json(AgencyReply { agency }))
}
