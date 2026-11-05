//! Listing the catalogue from the relational store with a cache-aside entry
//! in [`StateStore`].

use common::{Capabilities, RowExt as _};
use omnia_sdk::api::http::get;
use omnia_sdk::api::{Client, Context};
use omnia_sdk::sql::{DataType, Row};
use omnia_sdk::{Result, StateStore, TableStore, server_error};
use serde::{Deserialize, Serialize};

// The paths the part serves.
pub(crate) const CATALOGUE: &str = "/catalogue";

// The relational connection the part queries.
pub(crate) const CONNECTION: &str = "db";

// The state key and TTL of the cached catalogue.
pub(crate) const CATALOGUE_KEY: &str = "orders/catalogue";
const CATALOGUE_TTL_SECS: u64 = 60;

pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {
    axum::Router::new().route(CATALOGUE, get(list_catalogue))
}

/// Optional filter on the catalogue listing.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListCatalogue {
    /// Minimum price in cents (`0` when omitted).
    pub min_cents: Option<i64>,
}

/// One item in the catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogueItem {
    /// Item SKU.
    pub sku: String,
    /// Unit price in cents.
    pub cents: i64,
    /// Whether the item is currently in stock.
    pub in_stock: bool,
}

impl TryFrom<&Row> for CatalogueItem {
    type Error = omnia_sdk::Error;

    fn try_from(row: &Row) -> Result<Self> {
        Ok(Self {
            sku: row.get_str("sku")?.to_owned(),
            cents: row.get_i64("cents")?,
            in_stock: row.get_bool("in_stock")?,
        })
    }
}

/// The catalogue items matching the query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogueReply {
    /// Matching items ordered by SKU.
    pub items: Vec<CatalogueItem>,
}

pub(crate) async fn list_catalogue<P>(
    input: ListCatalogue, context: Context<P>,
) -> Result<CatalogueReply>
where
    P: StateStore + TableStore,
{
    let provider = context.provider();
    let min_cents = input.min_cents.unwrap_or(0);

    if min_cents == 0
        && let Some(bytes) = StateStore::get(provider, CATALOGUE_KEY).await?
        && let Ok(items) = serde_json::from_slice::<Vec<CatalogueItem>>(&bytes)
    {
        return Ok(CatalogueReply { items });
    }

    let rows = TableStore::query(
        provider,
        CONNECTION.to_owned(),
        "SELECT sku, cents, in_stock FROM catalogue WHERE cents >= $1 ORDER BY sku ASC".to_owned(),
        vec![DataType::Int64(Some(min_cents))],
    )
    .await?;
    let items = rows.iter().map(CatalogueItem::try_from).collect::<Result<Vec<_>>>()?;

    if min_cents == 0 {
        let bytes = serde_json::to_vec(&items).map_err(|err| server_error!("{err}"))?;
        StateStore::set(provider, CATALOGUE_KEY, &bytes, Some(CATALOGUE_TTL_SECS)).await?;
    }

    Ok(CatalogueReply { items })
}
