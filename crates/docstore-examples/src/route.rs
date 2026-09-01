//! Route collection: create, get, and a combined filter query.
//!
//! The list handler covers the filter shapes the stop query does not:
//! `or(contains, contains)` (name search across two fields), `in_list`
//! (route types), `negate` (type exclusion), and `negate(and(...))`
//! (agency + type exclusion — De Morgan negation).

use anyhow::Context as _;
use omnia_guest::api::Context;
use omnia_guest::document_store::{Document, Filter, QueryOptions, ScalarValue, SortField};
use omnia_guest::{DocumentStore, Result, not_found};
use serde::{Deserialize, Serialize};

use crate::{DocumentRecord, records};

/// Document collection holding the routes.
pub const COLLECTION: &str = "routes";

/// A GTFS-like route stored as one JSON document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    /// Operating agency id.
    pub agency_id: String,
    /// Short display name (e.g. `NEX`).
    pub route_short_name: String,
    /// Long display name (e.g. `Northern Express`).
    pub route_long_name: String,
    /// GTFS route type (`2` rail, `3` bus, `4` ferry, ...).
    pub route_type: i32,
    /// Brand colour as an RGB hex string.
    pub route_color: Option<String>,
}

/// Create a route with a caller-chosen id (fails when the id exists).
#[derive(Debug, Clone, Deserialize)]
pub struct CreateRouteRequest {
    /// Document id for the new route.
    pub id: String,
    /// The route record, flattened alongside the id in the JSON body.
    #[serde(flatten)]
    pub route: Route,
}

#[omnia_guest::handler]
async fn create_route_request<P>(
    input: CreateRouteRequest, context: Context<'_, P>,
) -> Result<DocumentRecord<Route>>
where
    P: DocumentStore,
{
    let document = Document {
        id: input.id.clone(),
        data: serde_json::to_vec(&input.route).context("serializing route")?,
    };
    DocumentStore::insert(context.provider, COLLECTION, &document).await?;

    Ok(DocumentRecord {
        id: input.id,
        document: input.route,
    })
}

/// Fetch one route by id.
#[derive(Debug, Clone, Deserialize)]
pub struct GetRouteRequest {
    /// Document id (path parameter).
    pub id: String,
}

#[omnia_guest::handler]
async fn get_route_request<P>(
    input: GetRouteRequest, context: Context<'_, P>,
) -> Result<DocumentRecord<Route>>
where
    P: DocumentStore,
{
    let document = DocumentStore::get(context.provider, COLLECTION, &input.id)
        .await?
        .ok_or_else(|| not_found!("route {} not found", input.id))?;
    let route = serde_json::from_slice(&document.data).context("deserializing route")?;

    Ok(DocumentRecord {
        id: document.id,
        document: route,
    })
}

/// Query routes with any combination of the supported filters.
///
/// Every field is optional; present fields are combined with AND. Results are
/// sorted by `route_short_name` ascending.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListRoutesRequest {
    /// Name search: `or(contains(route_short_name), contains(route_long_name))`.
    pub q: Option<String>,
    /// Comma-separated route types: `in_list` on `route_type`.
    pub types: Option<String>,
    /// Operating agency: `eq` on `agency_id`.
    pub agency: Option<String>,
    /// Type exclusion: `negate(eq(route_type, ...))`.
    pub exclude_type: Option<i32>,
    /// With [`Self::not_type`]: `negate(and(eq(agency_id), eq(route_type)))`.
    pub not_agency: Option<String>,
    /// With [`Self::not_agency`]: the route type to exclude.
    pub not_type: Option<i32>,
    /// Maximum documents per page.
    pub limit: Option<u32>,
    /// Continuation token from the previous page.
    pub continuation: Option<String>,
}

/// One page of matching routes.
#[derive(Debug, Clone, Serialize)]
pub struct RoutesReply {
    /// Matches sorted by `route_short_name` ascending.
    pub routes: Vec<DocumentRecord<Route>>,
    /// Token for the next page, when more matches remain.
    pub continuation: Option<String>,
}

#[omnia_guest::handler]
async fn list_routes_request<P>(
    input: ListRoutesRequest, context: Context<'_, P>,
) -> Result<RoutesReply>
where
    P: DocumentStore,
{
    let mut filters = Vec::new();

    if let Some(q) = &input.q {
        filters.push(Filter::or([
            Filter::contains("route_short_name", q),
            Filter::contains("route_long_name", q),
        ]));
    }
    if let Some(types) = &input.types {
        let values: Vec<ScalarValue> = types
            .split(',')
            .filter_map(|value| value.trim().parse::<i32>().ok())
            .map(ScalarValue::from)
            .collect();
        if !values.is_empty() {
            filters.push(Filter::in_list("route_type", values));
        }
    }
    if let Some(agency) = &input.agency {
        filters.push(Filter::eq("agency_id", agency.as_str()));
    }
    if let Some(exclude) = input.exclude_type {
        filters.push(Filter::negate(Filter::eq("route_type", exclude)));
    }
    if let (Some(agency), Some(route_type)) = (&input.not_agency, input.not_type) {
        filters.push(Filter::negate(Filter::and([
            Filter::eq("agency_id", agency.as_str()),
            Filter::eq("route_type", route_type),
        ])));
    }

    let filter = if filters.is_empty() { None } else { Some(Filter::and(filters)) };

    let result = DocumentStore::query(
        context.provider,
        COLLECTION,
        QueryOptions {
            filter,
            order_by: vec![SortField {
                field: "route_short_name".to_string(),
                descending: false,
            }],
            limit: input.limit,
            continuation: input.continuation,
            ..Default::default()
        },
    )
    .await?;

    Ok(RoutesReply {
        routes: records(&result.documents)?,
        continuation: result.continuation,
    })
}
