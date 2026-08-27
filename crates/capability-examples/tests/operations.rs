//! Integration tests driving each capability example through the mock
//! provider, invoked exactly as the guest invokes them.

mod provider;

use capability_examples::{AlertRequest, ArchiveRequest, NoteRequest, ReadingRequest};
use omnia_guest::api::{Client, Metadata};

use self::provider::MockProvider;

#[tokio::test]
async fn archive_object() {
    let provider = MockProvider::default();
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
    assert_eq!(provider.object("reports", "2026-07.json"), Some(br#"{"total":42}"#.to_vec()));
}

#[tokio::test]
async fn broadcast_alert() {
    let provider = MockProvider::default();
    let request = AlertRequest {
        channel: "ops".to_string(),
        message: "service degraded".to_string(),
        sockets: Some(vec!["socket-1".to_string()]),
    };

    Client::new("acme", provider.clone())
        .call(request, &Metadata::default())
        .await
        .expect("should succeed");

    let broadcasts = provider.broadcasts();
    assert_eq!(broadcasts.len(), 1);
    let (channel, data, sockets) = &broadcasts[0];
    assert_eq!(channel, "ops");
    assert_eq!(data, b"service degraded");
    assert_eq!(sockets.as_deref(), Some(["socket-1".to_string()].as_slice()));
}

#[tokio::test]
async fn upsert_note() {
    let provider = MockProvider::default();
    let request = NoteRequest {
        store: "notes".to_string(),
        id: "note-1".to_string(),
        body: serde_json::json!({ "text": "hello" }),
    };

    let reply = Client::new("acme", provider.clone())
        .call(request, &Metadata::default())
        .await
        .expect("should succeed");

    let stored = provider.document("notes", "note-1").expect("stored");
    assert_eq!(reply.size, stored.len());
    let body: serde_json::Value = serde_json::from_slice(&stored).expect("valid JSON");
    assert_eq!(body, serde_json::json!({ "text": "hello" }));
}

#[tokio::test]
async fn record_reading() {
    let provider = MockProvider::default();
    let client = Client::new("acme", provider.clone());

    let first = ReadingRequest {
        connection: "telemetry".to_string(),
        sensor: "temp-1".to_string(),
        value: 21.5,
    };
    let reply = client.call(first, &Metadata::default()).await.expect("should succeed");
    assert_eq!(reply.affected, 1);
    assert_eq!(reply.rows, 1);

    let second = ReadingRequest {
        connection: "telemetry".to_string(),
        sensor: "temp-1".to_string(),
        value: 22.0,
    };
    let reply = client.call(second, &Metadata::default()).await.expect("should succeed");
    assert_eq!(reply.affected, 1);
    assert_eq!(reply.rows, 2);

    assert_eq!(
        provider.readings(),
        vec![("temp-1".to_string(), 21.5), ("temp-1".to_string(), 22.0)]
    );
}
