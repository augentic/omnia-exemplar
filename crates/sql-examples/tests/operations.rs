#![allow(missing_docs)]

//! Integration tests driving the SQL-example handlers through the spy mock
//! provider: the full CRUD flow with server-assigned ids, partial updates,
//! the referential check, the JOIN listing, and delete-with-404.

mod provider;

use omnia_guest::api::{Client, Metadata};
use sql_examples::{
    AgencyReply, CreateAgencyRequest, CreateFeedRequest, DeleteFeedRequest, GetAgencyRequest,
    ListAgenciesRequest, ListAgencyFeedsRequest, ListAllFeedsRequest, UpdateAgencyRequest,
};

use self::provider::MockProvider;

async fn create_agency(client: &Client<MockProvider>, name: &str) -> AgencyReply {
    let request = CreateAgencyRequest {
        name: name.to_string(),
        url: Some(format!("https://{name}.example.nz").to_lowercase()),
        timezone: Some("Pacific/Auckland".to_string()),
    };
    client.call(request, &Metadata::default()).await.expect("agency should be created")
}

#[tokio::test]
async fn agency_crud_round_trip() {
    let provider = MockProvider::default();
    let client = Client::new("acme", provider.clone());

    // Server-assigned ids: max + 1, starting from 1.
    let first = create_agency(&client, "Ritchies").await;
    assert_eq!(first.agency.agency_id, 1);
    let second = create_agency(&client, "Metro").await;
    assert_eq!(second.agency.agency_id, 2);

    // List: both rows, newest first.
    let reply = client
        .call(ListAgenciesRequest::default(), &Metadata::default())
        .await
        .expect("list should succeed");
    let ids: Vec<i64> = reply.agencies.iter().map(|agency| agency.agency_id).collect();
    assert_eq!(ids, [2, 1]);

    // List with a limit.
    let request = ListAgenciesRequest { limit: Some(1) };
    let reply = client.call(request, &Metadata::default()).await.expect("list should succeed");
    assert_eq!(reply.agencies.len(), 1);

    // Get by id, and 404 for a missing row.
    let request = GetAgencyRequest { id: 1 };
    let reply = client.call(request, &Metadata::default()).await.expect("agency 1 should exist");
    assert_eq!(reply.agency.name, "Ritchies");
    let request = GetAgencyRequest { id: 99 };
    let error = client.call(request, &Metadata::default()).await.expect_err("agency 99 is absent");
    assert_eq!(error.code(), "not_found");

    // Partial update: only the provided field is written; the reply is the
    // fetched-after-update row.
    let request = UpdateAgencyRequest {
        id: 1,
        name: Some("Ritchies Transport".to_string()),
        url: None,
        timezone: None,
    };
    let reply = client.call(request, &Metadata::default()).await.expect("update should succeed");
    assert_eq!(reply.agency.name, "Ritchies Transport");
    assert_eq!(reply.agency.url.as_deref(), Some("https://ritchies.example.nz"));
    let stored = provider.agency(1).expect("agency 1 should be stored");
    assert_eq!(stored.name, "Ritchies Transport");
    assert_eq!(stored.timezone.as_deref(), Some("Pacific/Auckland"));

    // An empty patch is a bad request, and updating a missing row is a 404.
    let request = UpdateAgencyRequest {
        id: 1,
        name: None,
        url: None,
        timezone: None,
    };
    let error = client.call(request, &Metadata::default()).await.expect_err("empty patch");
    assert_eq!(error.code(), "bad_request");
    let request = UpdateAgencyRequest {
        id: 99,
        name: Some("Ghost".to_string()),
        url: None,
        timezone: None,
    };
    let error = client.call(request, &Metadata::default()).await.expect_err("agency 99 is absent");
    assert_eq!(error.code(), "not_found");
}

#[tokio::test]
async fn feed_flow_with_join_and_delete() {
    let provider = MockProvider::default();
    let client = Client::new("acme", provider.clone());
    create_agency(&client, "Ritchies").await;

    // A feed for a missing agency is rejected before any row is written.
    let request = CreateFeedRequest {
        agency_id: 99,
        description: "Orphan feed".to_string(),
    };
    let error = client.call(request, &Metadata::default()).await.expect_err("agency 99 is absent");
    assert_eq!(error.code(), "not_found");

    // Server-assigned feed ids.
    let request = CreateFeedRequest {
        agency_id: 1,
        description: "Bus routes and schedules".to_string(),
    };
    let reply = client.call(request, &Metadata::default()).await.expect("feed should be created");
    assert_eq!(reply.feed.feed_id, 1);
    let request = CreateFeedRequest {
        agency_id: 1,
        description: "Ferry timetables".to_string(),
    };
    let reply = client.call(request, &Metadata::default()).await.expect("feed should be created");
    assert_eq!(reply.feed.feed_id, 2);

    // Per-agency listing.
    let request = ListAgencyFeedsRequest { agency_id: 1 };
    let reply = client.call(request, &Metadata::default()).await.expect("list should succeed");
    assert_eq!(reply.feeds.len(), 2);

    // The JOIN listing carries the aliased agency columns.
    let reply = client
        .call(ListAllFeedsRequest::default(), &Metadata::default())
        .await
        .expect("joined list should succeed");
    assert_eq!(reply.feeds.len(), 2);
    for feed in &reply.feeds {
        assert_eq!(feed.agency_name, "Ritchies");
        assert_eq!(feed.agency_url.as_deref(), Some("https://ritchies.example.nz"));
        assert_eq!(feed.agency_timezone.as_deref(), Some("Pacific/Auckland"));
    }

    // Delete: rows affected drives the reply; a second delete is a 404.
    let request = DeleteFeedRequest { id: 1 };
    let reply = client.call(request.clone(), &Metadata::default()).await.expect("should delete");
    assert_eq!(reply.feed_id, 1);
    assert!(provider.feed(1).is_none());
    let error = client.call(request, &Metadata::default()).await.expect_err("already deleted");
    assert_eq!(error.code(), "not_found");
}

#[tokio::test]
async fn handlers_emit_parameterized_orm_sql() {
    let provider = MockProvider::default();
    let client = Client::new("acme", provider.clone());

    create_agency(&client, "Ritchies").await;
    let request = UpdateAgencyRequest {
        id: 1,
        name: Some("Ritchies Transport".to_string()),
        url: None,
        timezone: None,
    };
    client.call(request, &Metadata::default()).await.expect("update should succeed");

    let statements = provider.statements();

    // Every handler starts by ensuring the schema through TableStore::exec.
    assert!(statements[0].starts_with("CREATE TABLE IF NOT EXISTS agency"));
    assert!(statements[1].starts_with("CREATE TABLE IF NOT EXISTS feed"));

    // The insert names every entity column and binds values as parameters.
    let insert = statements
        .iter()
        .find(|sql| sql.starts_with("INSERT INTO \"agency\""))
        .expect("an agency insert should have run");
    for column in ["agency_id", "name", "url", "timezone", "created_at"] {
        assert!(insert.contains(&format!("\"{column}\"")), "insert should name {column}");
    }
    assert!(insert.contains("VALUES ($1, $2, $3, $4, $5)"), "insert should be parameterized");

    // The partial update sets only the provided column and filters by id.
    let update = statements
        .iter()
        .find(|sql| sql.starts_with("UPDATE \"agency\""))
        .expect("an agency update should have run");
    assert!(update.contains("SET \"name\" = $1"), "update should set only the name");
    assert!(!update.contains("\"timezone\""), "update should not touch unset columns");
    assert!(
        update.contains("WHERE (\"agency\".\"agency_id\") = ($2)"),
        "update should filter by id"
    );
}
