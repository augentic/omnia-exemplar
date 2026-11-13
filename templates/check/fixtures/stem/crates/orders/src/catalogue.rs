//! Listing the catalogue from the relational store with a cache-aside entry
//! in [`StateStore`].

use std::time::Duration;

use axum::Json;
use axum::extract::Query;
use axum::routing::get;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::sql::{FromRow, Value};
use omnia_sdk::{Result, SqlDb, SqlDbExt as _, StateStore, StateStoreExt as _};
use serde::{Deserialize, Serialize};

// The paths the part serves.
pub(crate) const CATALOGUE: &str = "/catalogue";

// The state key and TTL of the cached catalogue.
pub(crate) const CATALOGUE_KEY: &str = "orders.catalogue";
const CATALOGUE_TTL: Duration = Duration::from_secs(60);

pub(crate) fn handlers<P: crate::Requires>() -> Handlers<P> {
    Handlers::new().route(CATALOGUE, get(list_catalogue))
}

/// Optional filter on the catalogue listing.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListCatalogue {
    /// Minimum price in cents (`0` when omitted).
    pub min_cents: Option<i64>,
}

/// One item in the catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct CatalogueItem {
    /// Item SKU.
    pub sku: String,
    /// Unit price in cents.
    pub cents: i64,
    /// Whether the item is currently in stock.
    pub in_stock: bool,
}

/// The catalogue items matching the query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogueReply {
    /// Matching items ordered by SKU.
    pub items: Vec<CatalogueItem>,
}

pub(crate) async fn list_catalogue<P>(
    context: Context<P>, Query(input): Query<ListCatalogue>,
) -> Result<Json<CatalogueReply>>
where
    P: SqlDb + StateStore,
{
    let min_cents = input.min_cents.unwrap_or(0);

    if min_cents == 0
        && let Some(items) = context.state().get_json(CATALOGUE_KEY).await?
    {
        return Ok(Json(CatalogueReply { items }));
    }

    let items: Vec<CatalogueItem> = context
        .sql()
        .query_as(
            "SELECT sku, cents, in_stock FROM catalogue WHERE cents >= $1 ORDER BY sku ASC",
            &[Value::from(min_cents)],
        )
        .await?;

    if min_cents == 0 {
        context.state().set_json(CATALOGUE_KEY, &items, Some(CATALOGUE_TTL)).await?;
    }

    Ok(Json(CatalogueReply { items }))
}
