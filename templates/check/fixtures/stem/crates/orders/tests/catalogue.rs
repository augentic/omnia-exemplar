//! Querying the catalogue on the `default` database into `FromRow` items,
//! with a cache-aside entry in the `default` bucket in front of it.

mod support;

use axum::http::{Request, StatusCode};
use bytes::Bytes;
use omnia::{Provides, Runtime};
use omnia_test::host::sql::{Field, Row, Value};
use omnia_test::host::{self, Backends, ScriptedTables};
use omnia_wasi_http::WasiHttp;
use orders::catalogue::{CatalogueItem, CatalogueReply};
use support::boot;

fn item_row(sku: &str, cents: i64, in_stock: bool) -> Row {
    Row {
        fields: vec![
            Field {
                name: "sku".to_string(),
                value: Value::Str(Some(sku.to_string())),
            },
            Field {
                name: "cents".to_string(),
                value: Value::Int64(Some(cents)),
            },
            Field {
                name: "in_stock".to_string(),
                value: Value::Boolean(Some(in_stock)),
            },
        ],
    }
}

async fn get_catalogue<B: Provides<WasiHttp>>(runtime: &Runtime<B>) -> CatalogueReply {
    let request = Request::get("/catalogue").body(Bytes::new()).expect("request");
    let response = host::send(runtime, request).await.expect("request served");
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(response.body()).expect("catalogue reply")
}

#[tokio::test]
async fn queries_cache_serve() {
    let tables = ScriptedTables::default().on_query(
        |sql, params| {
            sql.starts_with("SELECT sku, cents, in_stock FROM catalogue")
                && matches!(params, [Value::Int64(Some(0))])
        },
        vec![item_row("cake", 450, true), item_row("tea", 300, false)],
    );
    let runtime = boot(Backends::defaults().await.sql(tables.clone())).await;

    let first = get_catalogue(&runtime).await;
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
    assert_eq!(tables.statements().len(), 1);

    // the second call served from the cache
    let second = get_catalogue(&runtime).await;
    runtime.shutdown();
    assert_eq!(second, first);
    assert_eq!(tables.statements().len(), 1);
}
