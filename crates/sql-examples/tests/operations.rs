#![allow(missing_docs)]

//! Integration tests driving the SQL-example handlers through a scripted
//! `TableStore`: server-assigned ids, partial updates, the referential
//! check, the JOIN listing, and delete-with-404.
//!
//! `ScriptedTables` answers statements rather than evaluating them, so each
//! test scripts the rows a handler's queries see and asserts the statements
//! the handler issued — the ORM-rendered SQL and its bound parameters.

use omnia_guest::api::{Client, Metadata};
use omnia_guest::orm::{DataType, Field, Row};
use omnia_test::guest::{ScriptedTables, Statement};
use sql_examples::{
    CreateAgencyRequest, CreateFeedRequest, DeleteFeedRequest, GetAgencyRequest,
    ListAgenciesRequest, ListAgencyFeedsRequest, ListAllFeedsRequest, UpdateAgencyRequest,
};

omnia_test::provider! {
    /// The handlers' one capability, as a scripted double.
    pub struct TestProvider: TableStore;
}

const OWNER: &str = "acme";

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

/// A feed row as the `FeedWithAgency` JOIN aliases it.
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

/// Every handler opens by ensuring the schema; every scripted store
/// acknowledges those two statements.
fn tables() -> ScriptedTables {
    ScriptedTables::default().on_exec(|sql, _| sql.starts_with("CREATE TABLE IF NOT EXISTS"), 0)
}

fn client(tables: ScriptedTables) -> Client<TestProvider> {
    Client::new(OWNER, TestProvider::default().tables(tables))
}

fn statements(client: &Client<TestProvider>) -> Vec<Statement> {
    client.provider().tables.statements()
}

/// The statements after the two schema `CREATE TABLE`s.
fn after_schema(client: &Client<TestProvider>) -> Vec<Statement> {
    let statements = statements(client);
    assert!(statements[0].sql.starts_with("CREATE TABLE IF NOT EXISTS agency"));
    assert!(statements[1].sql.starts_with("CREATE TABLE IF NOT EXISTS feed"));
    assert!(statements.iter().all(|statement| statement.connection == sql_examples::CONNECTION));
    statements[2..].to_vec()
}

fn create_agency(name: &str) -> CreateAgencyRequest {
    CreateAgencyRequest {
        name: name.to_string(),
        url: Some(format!("https://{name}.example.nz").to_lowercase()),
        timezone: Some("Pacific/Auckland".to_string()),
    }
}

// Probes for the newest id: `ORDER BY "<table>"."<pk>" DESC LIMIT 1`.
fn is_max_id_probe(table: &str, sql: &str, params: &[DataType]) -> bool {
    sql.contains(&format!("ORDER BY \"{table}\".\"{table}_id\" DESC")) && is_uint(params, 0, 1)
}

const AGENCY_FILTER: &str = "(\"agency\".\"agency_id\") = ($1)";

#[tokio::test]
async fn create_agency_assigns_next_id() {
    let client = client(
        tables()
            .on_query(
                |sql, params| is_max_id_probe("agency", sql, params),
                vec![agency_row(41, "Metro")],
            )
            .on_exec(|sql, _| sql.starts_with("INSERT INTO \"agency\""), 1),
    );

    let reply =
        client.call(create_agency("Ritchies"), &Metadata::default()).await.expect("created");
    assert_eq!(reply.agency.agency_id, 42);
    assert_eq!(reply.agency.name, "Ritchies");

    // The insert names every entity column, binds them as parameters, and
    // carries the assigned id first.
    let issued = after_schema(&client);
    let [probe, insert] = issued.as_slice() else {
        panic!("expected a probe then an insert");
    };
    assert!(probe.sql.starts_with("SELECT"));
    for column in ["agency_id", "name", "url", "timezone", "created_at"] {
        assert!(insert.sql.contains(&format!("\"{column}\"")), "insert should name {column}");
    }
    assert!(insert.sql.contains("VALUES ($1, $2, $3, $4, $5)"), "insert should be parameterized");
    assert_eq!(int_param(&insert.params, 0), Some(42));
    assert!(matches!(&insert.params[1], DataType::Str(Some(name)) if name == "Ritchies"));
}

#[tokio::test]
async fn create_agency_starts_from_one() {
    let client = client(
        tables()
            .on_query(|sql, params| is_max_id_probe("agency", sql, params), vec![])
            .on_exec(|sql, _| sql.starts_with("INSERT INTO \"agency\""), 1),
    );

    let reply =
        client.call(create_agency("Ritchies"), &Metadata::default()).await.expect("created");

    assert_eq!(reply.agency.agency_id, 1);
}

#[tokio::test]
async fn list_agencies_newest_first() {
    let client = client(tables().on_query(
        |sql, _| sql.contains("ORDER BY \"agency\".\"created_at\" DESC"),
        vec![agency_row(2, "Metro"), agency_row(1, "Ritchies")],
    ));

    let reply = client
        .call(ListAgenciesRequest::default(), &Metadata::default())
        .await
        .expect("list should succeed");
    let ids: Vec<i64> = reply.agencies.iter().map(|agency| agency.agency_id).collect();
    assert_eq!(ids, [2, 1]);

    // No limit was requested, so none is bound.
    let issued = after_schema(&client);
    let [list] = issued.as_slice() else {
        panic!("expected one select");
    };
    assert!(list.params.is_empty(), "{:?}", list.params);
    assert!(!list.sql.contains("LIMIT"));

    // A limit is bound as a parameter, not rendered into the SQL.
    let request = ListAgenciesRequest { limit: Some(1) };
    client.call(request, &Metadata::default()).await.expect("list should succeed");
    let limited = statements(&client).pop().expect("a select ran");
    assert!(limited.sql.contains("LIMIT"));
    assert!(is_uint(&limited.params, 0, 1), "{:?}", limited.params);
}

#[tokio::test]
async fn get_agency_by_id() {
    let client = client(
        tables()
            .on_query(
                |sql, params| sql.contains(AGENCY_FILTER) && int_param(params, 0) == Some(1),
                vec![agency_row(1, "Ritchies")],
            )
            .on_query(|sql, _| sql.contains(AGENCY_FILTER), vec![]),
    );

    let request = GetAgencyRequest { id: 1 };
    let reply = client.call(request, &Metadata::default()).await.expect("agency 1 should exist");
    assert_eq!(reply.agency.name, "Ritchies");
    assert_eq!(reply.agency.url.as_deref(), Some("https://ritchies.example.nz"));

    let request = GetAgencyRequest { id: 99 };
    let error = client.call(request, &Metadata::default()).await.expect_err("agency 99 is absent");
    assert_eq!(error.code(), "not_found");
}

#[tokio::test]
async fn update_agency_sets_only_provided_columns() {
    // The existence check and the fetch-after-update are the same select;
    // the scripted row is what the store holds after the update.
    let client = client(
        tables()
            .on_query(
                |sql, _| sql.contains(AGENCY_FILTER),
                vec![agency_row(1, "Ritchies Transport")],
            )
            .on_exec(|sql, _| sql.starts_with("UPDATE \"agency\" SET "), 1),
    );

    let request = UpdateAgencyRequest {
        id: 1,
        name: Some("Ritchies Transport".to_string()),
        url: None,
        timezone: None,
    };
    let reply = client.call(request, &Metadata::default()).await.expect("update should succeed");
    assert_eq!(reply.agency.name, "Ritchies Transport");
    assert_eq!(reply.agency.timezone.as_deref(), Some("Pacific/Auckland"));

    // Check, update, fetch again — and the update touches only the name.
    let issued = after_schema(&client);
    let [before, update, after] = issued.as_slice() else {
        panic!("expected fetch, update, fetch");
    };
    assert_eq!(before.sql, after.sql);
    assert!(update.sql.contains("SET \"name\" = $1"), "update should set only the name");
    assert!(!update.sql.contains("\"timezone\""), "update should not touch unset columns");
    assert!(
        update.sql.contains("WHERE (\"agency\".\"agency_id\") = ($2)"),
        "update should filter by id"
    );
    assert!(matches!(&update.params[0], DataType::Str(Some(name)) if name == "Ritchies Transport"));
    assert_eq!(int_param(&update.params, 1), Some(1));
}

#[tokio::test]
async fn update_agency_rejects_empty_patch_and_missing_row() {
    let client = client(
        tables()
            .on_query(is_agency_fetch_by_filter(1), vec![agency_row(1, "Ritchies")])
            .on_query(|sql, _| sql.contains(AGENCY_FILTER), vec![]),
    );

    // An empty patch is a bad request from the builder's guard, before any write.
    let request = UpdateAgencyRequest {
        id: 1,
        name: None,
        url: None,
        timezone: None,
    };
    let error = client.call(request, &Metadata::default()).await.expect_err("empty patch");
    assert_eq!(error.code(), "bad_request");
    assert!(!statements(&client).iter().any(|statement| statement.sql.starts_with("UPDATE")));

    // Updating a missing row is a 404.
    let request = UpdateAgencyRequest {
        id: 99,
        name: Some("Ghost".to_string()),
        url: None,
        timezone: None,
    };
    let error = client.call(request, &Metadata::default()).await.expect_err("agency 99 is absent");
    assert_eq!(error.code(), "not_found");
}

fn is_agency_fetch_by_filter(id: i64) -> impl Fn(&str, &[DataType]) -> bool + use<> {
    move |sql, params| sql.contains(AGENCY_FILTER) && int_param(params, 0) == Some(id)
}

#[tokio::test]
async fn create_feed_rejects_missing_agency_before_writing() {
    let client = client(tables().on_query(|sql, _| sql.contains(AGENCY_FILTER), vec![]));

    let request = CreateFeedRequest {
        agency_id: 99,
        description: "Orphan feed".to_string(),
    };
    let error = client.call(request, &Metadata::default()).await.expect_err("agency 99 is absent");

    assert_eq!(error.code(), "not_found");
    assert!(!statements(&client).iter().any(|statement| statement.sql.starts_with("INSERT")));
}

#[tokio::test]
async fn create_feed_assigns_next_id() {
    let client = client(
        tables()
            .on_query(is_agency_fetch_by_filter(1), vec![agency_row(1, "Ritchies")])
            .on_query(
                |sql, params| is_max_id_probe("feed", sql, params),
                vec![feed_row(1, 1, "Bus routes and schedules")],
            )
            .on_exec(|sql, _| sql.starts_with("INSERT INTO \"feed\""), 1),
    );

    let request = CreateFeedRequest {
        agency_id: 1,
        description: "Ferry timetables".to_string(),
    };
    let reply = client.call(request, &Metadata::default()).await.expect("feed should be created");
    assert_eq!(reply.feed.feed_id, 2);
    assert_eq!(reply.feed.agency_id, 1);

    let insert = statements(&client).pop().expect("an insert ran");
    assert!(insert.sql.contains("VALUES ($1, $2, $3, $4)"), "insert should be parameterized");
    assert_eq!(int_param(&insert.params, 0), Some(2));
    assert_eq!(int_param(&insert.params, 1), Some(1));
}

#[tokio::test]
async fn list_agency_feeds_filters_by_agency() {
    const FEED_FILTER: &str = "(\"feed\".\"agency_id\") = ($1)";
    let client = client(tables().on_query(
        |sql, params| sql.contains(FEED_FILTER) && int_param(params, 0) == Some(1),
        vec![feed_row(2, 1, "Ferry timetables"), feed_row(1, 1, "Bus routes and schedules")],
    ));

    let request = ListAgencyFeedsRequest { agency_id: 1 };
    let reply = client.call(request, &Metadata::default()).await.expect("list should succeed");

    let ids: Vec<i64> = reply.feeds.iter().map(|feed| feed.feed_id).collect();
    assert_eq!(ids, [2, 1]);
    let issued = after_schema(&client);
    let [list] = issued.as_slice() else {
        panic!("expected one select");
    };
    assert!(list.sql.contains("ORDER BY \"feed\".\"created_at\" DESC"), "{}", list.sql);
}

#[tokio::test]
async fn list_all_feeds_joins_agency_columns() {
    let client = client(tables().on_query(
        |sql, _| sql.contains("LEFT JOIN \"agency\""),
        vec![
            joined_row(2, "Ferry timetables", "Ritchies"),
            joined_row(1, "Bus routes", "Ritchies"),
        ],
    ));

    let reply = client
        .call(ListAllFeedsRequest::default(), &Metadata::default())
        .await
        .expect("joined list should succeed");

    // The aliased agency columns map onto the joined entity.
    assert_eq!(reply.feeds.len(), 2);
    for feed in &reply.feeds {
        assert_eq!(feed.agency_name, "Ritchies");
        assert_eq!(feed.agency_url.as_deref(), Some("https://ritchies.example.nz"));
        assert_eq!(feed.agency_timezone.as_deref(), Some("Pacific/Auckland"));
    }

    // The select renders the join, the aliases, the sort, and the default limit.
    let issued = after_schema(&client);
    let [list] = issued.as_slice() else {
        panic!("expected one select");
    };
    assert!(
        list.sql.contains(
            "LEFT JOIN \"agency\" ON (\"feed\".\"agency_id\") = (\"agency\".\"agency_id\")"
        ),
        "{}",
        list.sql
    );
    assert!(list.sql.contains("\"agency\".\"name\" AS \"agency_name\""), "{}", list.sql);
    assert!(list.sql.contains("ORDER BY \"feed\".\"created_at\" DESC"), "{}", list.sql);
    assert!(is_uint(&list.params, 0, 100), "default limit should be bound: {:?}", list.params);
}

#[tokio::test]
async fn delete_feed_is_not_found_on_zero_rows() {
    const FEED_ID_FILTER: &str = "(\"feed\".\"feed_id\") = ($1)";
    let client = client(
        tables()
            .on_exec(
                |sql, params| {
                    sql.starts_with("DELETE FROM \"feed\"") && int_param(params, 0) == Some(1)
                },
                1,
            )
            .on_exec(|sql, _| sql.starts_with("DELETE FROM \"feed\""), 0),
    );

    let request = DeleteFeedRequest { id: 1 };
    let reply = client.call(request, &Metadata::default()).await.expect("should delete");
    assert_eq!(reply.feed_id, 1);

    let request = DeleteFeedRequest { id: 2 };
    let error = client.call(request, &Metadata::default()).await.expect_err("nothing to delete");
    assert_eq!(error.code(), "not_found");

    let delete = statements(&client).pop().expect("a delete ran");
    assert!(delete.sql.contains(FEED_ID_FILTER), "{}", delete.sql);
}
