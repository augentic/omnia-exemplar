//! Integration tests for the agency and feed SQL handlers, driven through
//! the stem's router.

use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Method, Request, StatusCode};
use omnia_sdk::api::Client;
use omnia_sdk::sql::{DataType, Field, Row};
use omnia_test::guest::{self, Provider, ScriptedTables, Statement};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use sql::agency::{AgenciesReply, AgencyReply};
use sql::feed::{DeleteFeedReply, FeedReply, FeedsReply, FeedsWithAgencyReply};

fn field_int(name: &str, value: i64) -> Field {
    Field {
        name: name.to_string(),
        value: DataType::Int64(Some(value)),
    }
}

fn field_str(name: &str, value: Option<&str>) -> Field {
    Field {
        name: name.to_string(),
        value: DataType::Str(value.map(ToString::to_string)),
    }
}

fn agency_row(id: i64, name: &str) -> Row {
    Row {
        index: id.to_string(),
        fields: vec![
            field_int("agency_id", id),
            field_str("name", Some(name)),
            field_str("url", Some(&format!("https://{}.example.nz", name.to_lowercase()))),
            field_str("timezone", Some("Pacific/Auckland")),
            field_str("created_at", Some("2026-03-19 10:00:00")),
        ],
    }
}

fn feed_row(id: i64, agency_id: i64, description: &str) -> Row {
    Row {
        index: id.to_string(),
        fields: vec![
            field_int("feed_id", id),
            field_int("agency_id", agency_id),
            field_str("description", Some(description)),
            field_str("created_at", Some("2026-03-19 10:00:00")),
        ],
    }
}

fn joined_row(id: i64, description: &str, agency: &str) -> Row {
    let mut row = feed_row(id, 1, description);
    row.fields.extend([
        field_str("agency_name", Some(agency)),
        field_str("agency_url", Some(&format!("https://{}.example.nz", agency.to_lowercase()))),
        field_str("agency_timezone", Some("Pacific/Auckland")),
    ]);
    row
}

fn int_param(params: &[DataType], index: usize) -> Option<i64> {
    match params.get(index) {
        Some(DataType::Int64(value)) => *value,
        _ => None,
    }
}

fn is_uint(params: &[DataType], index: usize, expected: u64) -> bool {
    matches!(params.get(index), Some(DataType::Uint64(Some(value))) if *value == expected)
}

// Every handler first ensures the schema, so every scripted store
// acknowledges its two `CREATE TABLE` statements.
fn tables() -> ScriptedTables {
    ScriptedTables::default().on_exec(|sql, _| sql.starts_with("CREATE TABLE IF NOT EXISTS"), 0)
}

fn provider(tables: ScriptedTables) -> Provider {
    Provider::default().tables(tables)
}

fn statements(provider: &Provider) -> Vec<Statement> {
    provider.tables.statements()
}

fn after_schema(provider: &Provider) -> Vec<Statement> {
    let statements = statements(provider);
    assert!(statements[0].sql.starts_with("CREATE TABLE IF NOT EXISTS agency"));
    assert!(statements[1].sql.starts_with("CREATE TABLE IF NOT EXISTS feed"));
    assert!(statements.iter().all(|statement| statement.connection == "db"));
    statements[2..].to_vec()
}

fn agency_body(name: &str) -> Value {
    json!({
        "name": name,
        "url": format!("https://{name}.example.nz").to_lowercase(),
        "timezone": "Pacific/Auckland",
    })
}

async fn send(
    provider: &Provider, method: Method, uri: &str, body: Option<Value>,
) -> (StatusCode, Value) {
    let builder = Request::builder().method(method).uri(uri);
    let request = match body {
        Some(value) => {
            builder.header(CONTENT_TYPE, "application/json").body(Body::from(value.to_string()))
        }
        None => builder.body(Body::empty()),
    }
    .expect("request");
    let router = sql::handlers().with_state(Client::new(provider.clone()));
    let response = guest::send(&router, request).await;
    let json = serde_json::from_slice(response.body()).unwrap_or(Value::Null);
    (response.status(), json)
}

async fn ok<T: DeserializeOwned>(
    provider: &Provider, method: Method, uri: &str, body: Option<Value>,
) -> T {
    let (status, value) = send(provider, method, uri, body).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    serde_json::from_value(value).expect("reply")
}

// Probes for the newest id: `SELECT <pk> FROM <table> ORDER BY <pk> DESC LIMIT 1`.
fn is_max_id_probe(table: &str, sql: &str, params: &[DataType]) -> bool {
    sql == format!("SELECT {table}_id FROM {table} ORDER BY {table}_id DESC LIMIT 1")
        && params.is_empty()
}

const AGENCY_FILTER: &str = "WHERE agency_id = $1";

// The max-id probe SQL is the same for an empty and a populated table, so
// each table state is its own scripted store.
#[tokio::test]
async fn create_agency() {
    // Empty table: the first agency takes id 1.
    let empty = provider(
        tables()
            .on_query(|sql, params| is_max_id_probe("agency", sql, params), vec![])
            .on_exec(|sql, _| sql.starts_with("INSERT INTO agency"), 1),
    );

    let reply: AgencyReply =
        ok(&empty, Method::POST, "/examples/agencies", Some(agency_body("Ritchies"))).await;
    assert_eq!(reply.agency.agency_id, 1);

    // Populated table: the new agency takes the id after the newest.
    let populated = provider(
        tables()
            .on_query(
                |sql, params| is_max_id_probe("agency", sql, params),
                vec![agency_row(41, "Metro")],
            )
            .on_exec(|sql, _| sql.starts_with("INSERT INTO agency"), 1),
    );

    let reply: AgencyReply =
        ok(&populated, Method::POST, "/examples/agencies", Some(agency_body("Ritchies"))).await;
    assert_eq!(reply.agency.agency_id, 42);

    // The insert names every entity column, binds them as parameters, and
    // carries the assigned id first.
    let issued = after_schema(&populated);
    let [_, insert] = issued.as_slice() else {
        panic!("expected a probe then an insert");
    };
    for column in ["agency_id", "name", "url", "timezone", "created_at"] {
        assert!(insert.sql.contains(column), "insert should name {column}");
    }
    assert!(insert.sql.contains("VALUES ($1, $2, $3, $4, $5)"), "insert should be parameterized");
    assert_eq!(int_param(&insert.params, 0), Some(42));
    assert!(matches!(&insert.params[1], DataType::Str(Some(name)) if name == "Ritchies"));
}

#[tokio::test]
async fn list_agencies_limit() {
    let provider = provider(tables().on_query(
        |sql, _| sql.contains("ORDER BY created_at DESC"),
        vec![agency_row(2, "Metro"), agency_row(1, "Ritchies")],
    ));

    let _: AgenciesReply = ok(&provider, Method::GET, "/examples/agencies", None).await;

    // No limit was requested, so none is bound.
    let issued = after_schema(&provider);
    let [list] = issued.as_slice() else {
        panic!("expected one select");
    };
    assert!(list.params.is_empty(), "{:?}", list.params);
    assert!(!list.sql.contains("LIMIT"));

    // A limit is bound as a parameter, not rendered into the SQL.
    let _: AgenciesReply = ok(&provider, Method::GET, "/examples/agencies?limit=1", None).await;
    let limited = statements(&provider).pop().expect("a select ran");
    assert!(limited.sql.contains("LIMIT $1"));
    assert!(is_uint(&limited.params, 0, 1), "{:?}", limited.params);
}

#[tokio::test]
async fn get_agency() {
    let provider = provider(tables().on_query(agency_fetch(1), vec![agency_row(1, "Ritchies")]));

    let _: AgencyReply = ok(&provider, Method::GET, "/examples/agencies/1", None).await;
}

fn agency_fetch(id: i64) -> impl Fn(&str, &[DataType]) -> bool + use<> {
    move |sql, params| sql.contains(AGENCY_FILTER) && int_param(params, 0) == Some(id)
}

// Only the agency lookup is scripted, so a write after it panics.
#[tokio::test]
async fn missing_agency() {
    let provider = provider(tables().on_query(agency_fetch(99), vec![]));
    let patch = json!({ "name": "Ghost" });
    let feed = json!({ "description": "Orphan feed" });

    for (method, uri, body) in [
        (Method::GET, "/examples/agencies/99", None),
        (Method::PATCH, "/examples/agencies/99", Some(patch)),
        (Method::POST, "/examples/agencies/99/feeds", Some(feed)),
    ] {
        let request = format!("{method} {uri}");
        let (status, reply) = send(&provider, method, uri, body).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{request}");
        assert_eq!(reply["error"], "not_found", "{request}");
    }
}

#[tokio::test]
async fn update_agency_partial() {
    // The existence check and the fetch-after-update are the same select,
    // so one rule answers both.
    let provider = provider(
        tables()
            .on_query(
                |sql, _| sql.contains(AGENCY_FILTER),
                vec![agency_row(1, "Ritchies Transport")],
            )
            .on_exec(|sql, _| sql.starts_with("UPDATE agency SET "), 1),
    );

    let _: AgencyReply = ok(
        &provider,
        Method::PATCH,
        "/examples/agencies/1",
        Some(json!({ "name": "Ritchies Transport" })),
    )
    .await;

    // Check, update, fetch again — and the update touches only the name.
    let issued = after_schema(&provider);
    let [before, update, after] = issued.as_slice() else {
        panic!("expected fetch, update, fetch");
    };
    assert_eq!(before.sql, after.sql);
    assert!(update.sql.contains("SET name = $1"), "update should set only the name");
    assert!(!update.sql.contains("timezone"), "update should not touch unset columns");
    assert!(update.sql.contains("WHERE agency_id = $2"), "update should filter by id");
    assert!(matches!(&update.params[0], DataType::Str(Some(name)) if name == "Ritchies Transport"));
    assert_eq!(int_param(&update.params, 1), Some(1));
}

// No `UPDATE` rule is scripted, so a write before the rejection panics.
#[tokio::test]
async fn empty_patch() {
    let provider = provider(tables().on_query(agency_fetch(1), vec![agency_row(1, "Ritchies")]));

    let (status, body) =
        send(&provider, Method::PATCH, "/examples/agencies/1", Some(json!({}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "bad_request");
}

#[tokio::test]
async fn create_feed_existing() {
    let provider = provider(
        tables()
            .on_query(agency_fetch(1), vec![agency_row(1, "Ritchies")])
            .on_query(
                |sql, params| is_max_id_probe("feed", sql, params),
                vec![feed_row(1, 1, "Bus routes and schedules")],
            )
            .on_exec(|sql, _| sql.starts_with("INSERT INTO feed"), 1),
    );

    let reply: FeedReply = ok(
        &provider,
        Method::POST,
        "/examples/agencies/1/feeds",
        Some(json!({ "description": "Ferry timetables" })),
    )
    .await;
    assert_eq!(reply.feed.feed_id, 2);
    assert_eq!(reply.feed.agency_id, 1);

    let insert = statements(&provider).pop().expect("an insert ran");
    assert!(insert.sql.contains("VALUES ($1, $2, $3, $4)"), "insert should be parameterized");
    assert_eq!(int_param(&insert.params, 0), Some(2));
    assert_eq!(int_param(&insert.params, 1), Some(1));
}

#[tokio::test]
async fn feeds_by_agency() {
    const FEED_FILTER: &str = "FROM feed WHERE agency_id = $1";
    let provider = provider(tables().on_query(
        |sql, params| sql.contains(FEED_FILTER) && int_param(params, 0) == Some(1),
        vec![feed_row(2, 1, "Ferry timetables"), feed_row(1, 1, "Bus routes and schedules")],
    ));

    let _: FeedsReply = ok(&provider, Method::GET, "/examples/agencies/1/feeds", None).await;

    let issued = after_schema(&provider);
    let [list] = issued.as_slice() else {
        panic!("expected one select");
    };
    assert!(list.sql.contains("ORDER BY created_at DESC"), "{}", list.sql);
}

#[tokio::test]
async fn all_feeds() {
    let provider = provider(tables().on_query(
        |sql, _| sql.contains("LEFT JOIN agency"),
        vec![
            joined_row(2, "Ferry timetables", "Ritchies"),
            joined_row(1, "Bus routes", "Ritchies"),
        ],
    ));

    let _: FeedsWithAgencyReply = ok(&provider, Method::GET, "/examples/feeds", None).await;

    // The select renders the join, the aliases, the sort, and the default limit.
    let issued = after_schema(&provider);
    let [list] = issued.as_slice() else {
        panic!("expected one select");
    };
    assert!(
        list.sql.contains("LEFT JOIN agency ON feed.agency_id = agency.agency_id"),
        "{}",
        list.sql
    );
    for alias in [
        "agency.name AS agency_name",
        "agency.url AS agency_url",
        "agency.timezone AS agency_timezone",
    ] {
        assert!(list.sql.contains(alias), "{}", list.sql);
    }
    assert!(list.sql.contains("ORDER BY feed.created_at DESC"), "{}", list.sql);
    assert!(is_uint(&list.params, 0, 100), "default limit should be bound: {:?}", list.params);
}

#[tokio::test]
async fn delete_feed_missing() {
    const FEED_ID_FILTER: &str = "WHERE feed_id = $1";
    let provider = provider(
        tables()
            .on_exec(
                |sql, params| {
                    sql.starts_with("DELETE FROM feed") && int_param(params, 0) == Some(1)
                },
                1,
            )
            .on_exec(|sql, _| sql.starts_with("DELETE FROM feed"), 0),
    );

    let reply: DeleteFeedReply = ok(&provider, Method::DELETE, "/examples/feeds/1", None).await;
    assert_eq!(reply.feed_id, 1);

    let (status, body) = send(&provider, Method::DELETE, "/examples/feeds/2", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");

    let delete = statements(&provider).pop().expect("a delete ran");
    assert!(delete.sql.contains(FEED_ID_FILTER), "{}", delete.sql);
}
