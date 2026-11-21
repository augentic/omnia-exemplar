//! Listing the catalogue from the `default` [`sql`] database with a
//! cache-aside entry in the `default` [`keyvalue`] bucket.

use std::time::Duration;

use axum::Json;
use axum::extract::Query;
use axum::routing::get;
use omnia_sdk::sql::{FromRow, Value};
use omnia_sdk::{Handlers, Result, keyvalue, sql};
use serde::{Deserialize, Serialize};

// The paths the part serves.
pub(crate) const CATALOGUE: &str = "/catalogue";

// The state key and TTL of the cached catalogue.
pub(crate) const CATALOGUE_KEY: &str = "orders.catalogue";
const CATALOGUE_TTL: Duration = Duration::from_secs(60);

pub(crate) fn handlers() -> Handlers {
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

pub(crate) async fn list_catalogue(
    Query(input): Query<ListCatalogue>,
) -> Result<Json<CatalogueReply>> {
    let min_cents = input.min_cents.unwrap_or(0);
    let cache = keyvalue::default().await?;

    if min_cents == 0
        && let Some(items) = cache.get_json(CATALOGUE_KEY).await?
    {
        return Ok(Json(CatalogueReply { items }));
    }

    let items: Vec<CatalogueItem> = sql::default()
        .await?
        .query_as(
            "SELECT sku, cents, in_stock FROM catalogue WHERE cents >= $1 ORDER BY sku ASC",
            &[Value::from(min_cents)],
        )
        .await?;

    if min_cents == 0 {
        cache.set_json(CATALOGUE_KEY, &items, Some(CATALOGUE_TTL)).await?;
    }

    Ok(Json(CatalogueReply { items }))
}
