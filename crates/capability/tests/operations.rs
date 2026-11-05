//! Integration tests driving each capability example through its router and
//! `omnia_test` doubles.

use axum::body::{Body, to_bytes};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use capability::blob::{ArchiveReply, ArchiveRequest};
use capability::broadcast::AlertRequest;
use capability::document::{NoteReply, NoteRequest};
use capability::table::{ReadingReply, ReadingRequest, sql};
use omnia_sdk::DocumentStore as _;
use omnia_sdk::api::Client;
use omnia_sdk::sql::{DataType, Field, Row};
use omnia_test::guest::{Broadcasted, Provider, ScriptedTables};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tower::ServiceExt as _;

async fn post<I: Serialize + Sync, O: DeserializeOwned>(
    provider: &Provider, path: &str, input: &I,
) -> O {
    let request = Request::post(path)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(input).expect("serialize")))
        .expect("request");
    let router = capability::routes().with_state(Client::new(provider.clone()));
    let response = router.oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    serde_json::from_slice(&bytes).expect("deserialize")
}

#[tokio::test]
async fn archive_object() {
    let provider = Provider::default();
    let request = ArchiveRequest {
        container: "reports".to_string(),
        name: "2026-07.json".to_string(),
        payload: r#"{"total":42}"#.to_string(),
    };

    let reply: ArchiveReply = post(&provider, "/examples/archive", &request).await;

    assert_eq!(reply.size, 12);
    assert_eq!(
        provider.storage.object("reports", "2026-07.json"),
        Some(br#"{"total":42}"#.to_vec())
    );
}

#[tokio::test]
async fn broadcast_alert() {
    let provider = Provider::default();
    let request = AlertRequest {
        channel: "ops".to_string(),
        message: "service degraded".to_string(),
        sockets: Some(vec!["socket-1".to_string()]),
    };

    let reply: String = post(&provider, "/examples/alert", &request).await;

    assert_eq!(reply, "OK");
    assert_eq!(
        provider.broadcast.broadcasts(),
        [Broadcasted {
            name: "ops".to_string(),
            data: b"service degraded".to_vec(),
            sockets: Some(vec!["socket-1".to_string()]),
        }]
    );
}

#[tokio::test]
async fn upsert_note() {
    let provider = Provider::default();
    let request = NoteRequest {
        store: "notes".to_string(),
        id: "note-1".to_string(),
        body: serde_json::json!({ "text": "hello" }),
    };

    let reply: NoteReply = post(&provider, "/examples/note", &request).await;

    let stored = provider.docs.get("notes", "note-1").await.expect("get").expect("stored");
    assert_eq!(reply.size, stored.data.len());
    let body: serde_json::Value = serde_json::from_slice(&stored.data).expect("valid JSON");
    assert_eq!(body, serde_json::json!({ "text": "hello" }));
}

#[tokio::test]
async fn record_reading() {
    // The insert is acknowledged and the sensor already has one reading, so
    // the count the handler reports back is two.
    let earlier = Row {
        index: "0".to_string(),
        fields: vec![
            Field {
                name: "sensor".to_string(),
                value: DataType::Str(Some("temp-1".to_string())),
            },
            Field {
                name: "value".to_string(),
                value: DataType::Double(Some(21.5)),
            },
        ],
    };
    let provider = Provider::default().tables(
        ScriptedTables::default()
            .on_exec(|sql, _| sql == sql::INSERT, 1)
            .on_query(|sql, _| sql == sql::SELECT, vec![earlier.clone(), earlier]),
    );

    let request = ReadingRequest {
        connection: "telemetry".to_string(),
        sensor: "temp-1".to_string(),
        value: 22.0,
    };
    let reply: ReadingReply = post(&provider, "/examples/reading", &request).await;

    assert_eq!(reply.affected, 1);
    assert_eq!(reply.rows, 2);

    // Both statements went to the named connection with the sensor bound.
    let statements = provider.tables.statements();
    assert_eq!(statements.len(), 2);
    assert!(statements.iter().all(|statement| statement.connection == "telemetry"));
    assert_eq!(statements[0].sql, sql::INSERT);
    assert!(matches!(
        statements[0].params.as_slice(),
        [DataType::Str(Some(sensor)), DataType::Double(Some(value))]
            if sensor == "temp-1" && value.to_bits() == 22.0_f64.to_bits()
    ));
    assert_eq!(statements[1].sql, sql::SELECT);
    assert!(
        matches!(statements[1].params.as_slice(), [DataType::Str(Some(sensor))] if sensor == "temp-1")
    );
}
