//! Integration tests driving each capability example through the stem's
//! component over `omnia_test::host` backends.

use bytes::Bytes;
use capability::blob::{ArchiveReply, ArchiveRequest};
use capability::broadcast::AlertRequest;
use capability::document::{NoteReply, NoteRequest};
use capability::table::{ReadingReply, ReadingRequest, sql};
use http::header::CONTENT_TYPE;
use http::{Request, StatusCode};
use omnia::{Connect as _, Provides, Runtime};
use omnia_test::host::sql::{Row, Value};
use omnia_test::host::{
    Backends, Broadcasted, Deployment, ScriptedTables, Sink, precompiled, send,
};
use omnia_wasi_blobstore::WasiBlobStore;
use omnia_wasi_docstore::{ConnectOptions, DocStoreClient as _, DocStoreDefault, WasiDocStore};
use omnia_wasi_http::WasiHttp;
use omnia_wasi_messaging::WasiMessaging;
use omnia_wasi_otel::WasiOtel;
use omnia_wasi_sql::WasiSql;
use omnia_wasi_websocket::WasiWebSocket;
use serde::Serialize;
use serde::de::DeserializeOwned;

include!(concat!(env!("OUT_DIR"), "/gen.rs"));

async fn boot<B>(backends: B) -> Runtime<B>
where
    B: Provides<WasiBlobStore>
        + Provides<WasiDocStore>
        + Provides<WasiHttp>
        + Provides<WasiMessaging>
        + Provides<WasiOtel>
        + Provides<WasiSql>
        + Provides<WasiWebSocket>,
{
    let guest = precompiled(CAPABILITY_GUEST).expect("guest precompiles");
    Deployment::new()
        .guest("capability", guest)
        .boot(backends, |deployment| {
            deployment.host::<WasiBlobStore>()?;
            deployment.host::<WasiDocStore>()?;
            deployment.host::<WasiHttp>()?;
            deployment.host::<WasiMessaging>()?;
            deployment.host::<WasiOtel>()?;
            deployment.host::<WasiSql>()?;
            deployment.host::<WasiWebSocket>()?;
            Ok(())
        })
        .await
        .expect("runtime boots")
}

async fn post<B, I, O>(runtime: &Runtime<B>, path: &str, input: &I) -> O
where
    B: Provides<WasiHttp>,
    I: Serialize + Sync,
    O: DeserializeOwned,
{
    let request = Request::post(path)
        .header(CONTENT_TYPE, "application/json")
        .body(Bytes::from(serde_json::to_vec(input).expect("serialize")))
        .expect("request");
    let response = send(runtime, request).await.expect("request served");
    assert_eq!(response.status(), StatusCode::OK, "{:?}", response.body());
    serde_json::from_slice(response.body()).expect("deserialize")
}

#[tokio::test]
async fn archive_object() {
    let backends = Backends::defaults().await;
    let runtime = boot(backends.clone()).await;
    let request = ArchiveRequest {
        container: "reports".to_string(),
        name: "2026-07.json".to_string(),
        payload: r#"{"total":42}"#.to_string(),
    };

    let reply: ArchiveReply = post(&runtime, "/examples/archive", &request).await;
    runtime.shutdown();

    assert_eq!(reply.size, 12);
    assert_eq!(backends.object("reports", "2026-07.json").await, Some(br#"{"total":42}"#.to_vec()));
}

#[tokio::test]
async fn broadcast_alert() {
    let sink = Sink::default();
    let runtime = boot(Backends::defaults().await.websocket(sink.clone())).await;
    let targeted = AlertRequest {
        message: "service degraded".to_string(),
        sockets: Some(vec!["socket-1".to_string()]),
    };
    let everyone = AlertRequest {
        message: "service restored".to_string(),
        sockets: None,
    };

    let reply: String = post(&runtime, "/examples/alert", &targeted).await;
    assert_eq!(reply, "OK");
    let reply: String = post(&runtime, "/examples/alert", &everyone).await;
    assert_eq!(reply, "OK");
    runtime.shutdown();

    assert_eq!(
        sink.broadcasts(),
        [
            Broadcasted {
                data: b"service degraded".to_vec(),
                sockets: Some(vec!["socket-1".to_string()]),
                except: Vec::new(),
            },
            Broadcasted {
                data: b"service restored".to_vec(),
                sockets: None,
                except: Vec::new(),
            },
        ]
    );
}

#[tokio::test]
async fn upsert_note() {
    let options = ConnectOptions {
        collections: vec!["notes".to_string()],
    };
    let docs = DocStoreDefault::connect_with(options).await.expect("docstore connects");
    let runtime = boot(Backends::defaults().await.docstore(docs.clone())).await;
    let request = NoteRequest {
        store: "notes".to_string(),
        id: "note-1".to_string(),
        body: serde_json::json!({ "text": "hello" }),
    };

    let reply: NoteReply = post(&runtime, "/examples/note", &request).await;
    runtime.shutdown();

    let stored = docs.get("notes".to_string(), "note-1".to_string()).await.expect("get");
    let stored = stored.expect("stored");
    assert_eq!(reply.size, stored.data.len());
    let body: serde_json::Value = serde_json::from_slice(&stored.data).expect("valid JSON");
    assert_eq!(body, serde_json::json!({ "text": "hello" }));
}

#[tokio::test]
async fn record_reading() {
    // The handler reports only how many rows the select returns, so the two
    // scripted rows carry no fields.
    let tables = ScriptedTables::default()
        .on_exec(|statement, _| statement == sql::INSERT, 1)
        .on_query(|statement, _| statement == sql::SELECT, vec![Row { fields: Vec::new() }; 2]);
    let runtime = boot(Backends::defaults().await.sql(tables.clone())).await;

    let request = ReadingRequest {
        connection: "default".to_string(),
        sensor: "temp-1".to_string(),
        value: 22.0,
    };
    let reply: ReadingReply = post(&runtime, "/examples/reading", &request).await;
    runtime.shutdown();

    assert_eq!(reply.affected, 1);
    assert_eq!(reply.rows, 2);

    // The insert ran before the select, both on the named binding with the
    // sensor bound.
    let statements = tables.statements();
    assert_eq!(statements.len(), 2);
    assert!(statements.iter().all(|statement| statement.binding == "default"));
    let sensor = Value::Str(Some("temp-1".to_string()));
    assert_eq!(statements[0].params, [sensor.clone(), Value::Double(Some(22.0))]);
    assert_eq!(statements[1].params, [sensor]);
}
