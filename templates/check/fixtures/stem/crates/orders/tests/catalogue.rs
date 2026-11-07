//! Querying the catalogue over `TableStore` into `FromRow` items, with a
//! `StateStore` cache-aside in front of it.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use omnia_sdk::api::Client;
use omnia_sdk::sql::{DataType, Field, Row};
use omnia_test::guest::{Provider, ScriptedTables, send};
use orders::catalogue::{CatalogueItem, CatalogueReply};

fn item_row(sku: &str, cents: i64, in_stock: bool) -> Row {
    Row {
        index: sku.to_string(),
        fields: vec![
            Field {
                name: "sku".to_string(),
                value: DataType::Str(Some(sku.to_string())),
            },
            Field {
                name: "cents".to_string(),
                value: DataType::Int64(Some(cents)),
            },
            Field {
                name: "in_stock".to_string(),
                value: DataType::Boolean(Some(in_stock)),
            },
        ],
    }
}

async fn get_catalogue(provider: &Provider) -> CatalogueReply {
    let request = Request::get("/catalogue").body(Body::empty()).expect("request");
    let router = orders::handlers().with_state(Client::new(provider.clone()));
    let response = send(&router, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(response.body()).expect("catalogue reply")
}

#[tokio::test]
async fn queries_cache_serve() {
    let tables = ScriptedTables::default().on_query(
        |sql, params| {
            sql.starts_with("SELECT sku, cents, in_stock FROM catalogue")
                && matches!(params, [DataType::Int64(Some(0))])
        },
        vec![item_row("cake", 450, true), item_row("tea", 300, false)],
    );
    let provider = Provider::default().tables(tables);

    let first = get_catalogue(&provider).await;
    assert_eq!(
        first.items,
        [
            CatalogueItem {
                sku: "cake".to_string(),
                cents: 450,
                in_stock: true,
            },
            CatalogueItem {
                sku: "tea".to_string(),
                cents: 300,
                in_stock: false,
            },
        ]
    );
    assert_eq!(provider.tables.statements().len(), 1);

    // The second call is answered from `StateStore` without another SQL query.
    let second = get_catalogue(&provider).await;
    assert_eq!(second, first);
    assert_eq!(provider.tables.statements().len(), 1);
}
