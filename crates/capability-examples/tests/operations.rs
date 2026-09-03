//! Integration tests driving each capability example through `omnia_test`
//! doubles, invoked exactly as the guest invokes them.

use capability_examples::table::sql;
use capability_examples::{AlertRequest, ArchiveRequest, NoteRequest, ReadingRequest};
use omnia_guest::DocumentStore as _;
use omnia_guest::api::{Client, Metadata};
use omnia_guest::orm::{DataType, Field, Row};
use omnia_test::guest::{Broadcasted, ScriptedTables};

omnia_test::provider! {
    /// The union of the examples' capability lists, as doubles.
    pub struct TestProvider: BlobStore + Broadcast + DocumentStore + TableStore;
}

#[tokio::test]
async fn archive_object() {
    let provider = TestProvider::default();
    let request = ArchiveRequest {
        container: "reports".to_string(),
        name: "2026-07.json".to_string(),
        payload: r#"{"total":42}"#.to_string(),
    };

    let reply = Client::new("acme", provider.clone())
        .call(request, &Metadata::default())
        .await
        .expect("should succeed");

    assert_eq!(reply.size, 12);
    assert_eq!(
        provider.storage.object("reports", "2026-07.json"),
        Some(br#"{"total":42}"#.to_vec())
    );
}

#[tokio::test]
async fn broadcast_alert() {
    let provider = TestProvider::default();
    let request = AlertRequest {
        channel: "ops".to_string(),
        message: "service degraded".to_string(),
        sockets: Some(vec!["socket-1".to_string()]),
    };

    Client::new("acme", provider.clone())
        .call(request, &Metadata::default())
        .await
        .expect("should succeed");

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
    let provider = TestProvider::default();
    let request = NoteRequest {
        store: "notes".to_string(),
        id: "note-1".to_string(),
        body: serde_json::json!({ "text": "hello" }),
    };

    let reply = Client::new("acme", provider.clone())
        .call(request, &Metadata::default())
        .await
        .expect("should succeed");

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
    let provider = TestProvider::default().tables(
        ScriptedTables::default()
            .on_exec(|sql, _| sql == sql::INSERT, 1)
            .on_query(|sql, _| sql == sql::SELECT, vec![earlier.clone(), earlier]),
    );

    let request = ReadingRequest {
        connection: "telemetry".to_string(),
        sensor: "temp-1".to_string(),
        value: 22.0,
    };
    let reply = Client::new("acme", provider.clone())
        .call(request, &Metadata::default())
        .await
        .expect("should succeed");

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
