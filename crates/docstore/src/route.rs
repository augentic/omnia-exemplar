//! Route collection: create, get, and a combined filter query.
//!
//! The list handler covers the filter shapes the stop query does not:
//! `or(contains, contains)` (name search across two fields), `in_list`
//! (route types), `negate` (type exclusion), and `negate(and(...))`
//! (agency + type exclusion — De Morgan negation).

use axum::Json;
use axum::extract::{Path, Query};
use axum::routing::get;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::docstore::{Document, Exists, Filter, ScalarValue, SortField};
use omnia_sdk::{DocumentDb, Result, bad_request, not_found};
use serde::{Deserialize, Serialize};

use crate::stop::{CreateError, DocumentRecord, records};

pub(crate) const ROUTES: &str = "/examples/routes";
pub(crate) const ROUTE: &str = "/examples/routes/{id}";
pub(crate) const COLLECTION: &str = "routes";

pub(crate) fn handlers<P: crate::Requires>() -> Handlers<P> {
    Handlers::new().route(ROUTES, get(list_routes).post(create_route)).route(ROUTE, get(get_route))
}

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

/// Create a route with a caller-chosen id (`409 Conflict` when the id exists).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRouteRequest {
    /// Document id for the new route.
    pub id: String,
    /// The route record, flattened alongside the id in the JSON body.
    #[serde(flatten)]
    pub route: Route,
}

pub(crate) async fn create_route<P>(
    context: Context<P>, Json(input): Json<CreateRouteRequest>,
) -> std::result::Result<Json<DocumentRecord<Route>>, CreateError>
where
    P: DocumentDb,
{
    let document = Document::json(&input.id, &input.route)?;
    if context.docs().insert(COLLECTION, &document).await? == Err(Exists) {
        return Err(CreateError::Exists { id: input.id });
    }

    Ok(Json(DocumentRecord {
        id: input.id,
        document: input.route,
    }))
}

pub(crate) async fn get_route<P>(
    context: Context<P>, Path(id): Path<String>,
) -> Result<Json<DocumentRecord<Route>>>
where
    P: DocumentDb,
{
    let document = context
        .docs()
        .get(COLLECTION, &id)
        .await?
        .ok_or_else(|| not_found!("route {id} not found"))?;
    let route = document.decode()?;

    Ok(Json(DocumentRecord {
        id: document.id,
        document: route,
    }))
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
    ///
    /// More than [`Filter::MAX_LIST_LEN`] types is a `400`.
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutesReply {
    /// Matches sorted by `route_short_name` ascending.
    pub routes: Vec<DocumentRecord<Route>>,
    /// Token for the next page, when more matches remain.
    pub continuation: Option<String>,
}

pub(crate) async fn list_routes<P>(
    context: Context<P>, Query(input): Query<ListRoutesRequest>,
) -> Result<Json<RoutesReply>>
where
    P: DocumentDb,
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
        if values.len() > Filter::MAX_LIST_LEN {
            return Err(bad_request!("at most {} route types", Filter::MAX_LIST_LEN));
        }
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

    let query = omnia_sdk::docstore::Query {
        filter,
        order_by: vec![SortField {
            field: "route_short_name".to_string(),
            descending: false,
        }],
        limit: input.limit,
        after: input.continuation,
        ..Default::default()
    };
    let page = context.docs().query(COLLECTION, &query).await?;

    Ok(Json(RoutesReply {
        routes: records(&page.items)?,
        continuation: page.next,
    }))
}
