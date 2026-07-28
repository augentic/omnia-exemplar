//! Integration tests driving each capability example through the mock
//! provider, invoked exactly as the guests invoke them.

mod provider;

use capability_examples::{AlertRequest, ArchiveRequest, NoteRequest, ReadingRequest};
use omnia_guest::api::{Invocation, Invoker};

use self::provider::MockProvider;

#[tokio::test]
async fn archive_object() {
    let provider = MockProvider::default();
    let request = ArchiveRequest {
        container: "reports".to_string(),
        name: "2026-07.json".to_string(),
        payload: r#"{"total":42}"#.to_string(),
    };

    let reply = Invoker::new("acme", provider.clone())
        .invoke::<ArchiveRequest>(Invocation::new(request))
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

    Invoker::new("acme", provider.clone())
        .invoke::<AlertRequest>(Invocation::new(request))
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

    let reply = Invoker::new("acme", provider.clone())
        .invoke::<NoteRequest>(Invocation::new(request))
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
    let invoker = Invoker::new("acme", provider.clone());

    let first = ReadingRequest {
        connection: "telemetry".to_string(),
        sensor: "temp-1".to_string(),
        value: 21.5,
    };
    let reply =
        invoker.invoke::<ReadingRequest>(Invocation::new(first)).await.expect("should succeed");
    assert_eq!(reply.affected, 1);
    assert_eq!(reply.rows, 1);

    let second = ReadingRequest {
        connection: "telemetry".to_string(),
        sensor: "temp-1".to_string(),
        value: 22.0,
    };
    let reply =
        invoker.invoke::<ReadingRequest>(Invocation::new(second)).await.expect("should succeed");
    assert_eq!(reply.affected, 1);
    assert_eq!(reply.rows, 2);

    assert_eq!(
        provider.readings(),
        vec![("temp-1".to_string(), 21.5), ("temp-1".to_string(), 22.0)]
    );
}
