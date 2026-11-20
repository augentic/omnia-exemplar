//! Static tests for the Tally APC connector driven through its component.

use bytes::Bytes;
use common::topic;
use http::header::CONTENT_TYPE;
use http::{Request, StatusCode};
use omnia_test::host::{Backends, Deployment, Sink, precompiled, send};
use omnia_wasi_http::WasiHttp;
use omnia_wasi_messaging::WasiMessaging;
use omnia_wasi_otel::WasiOtel;
use serde_json::{Value, json};

include!(concat!(env!("OUT_DIR"), "/gen.rs"));

const TALLY_MESSAGE: &[u8] = include_bytes!("../data/tally-message.json");
const TALLY_NO_DEVICE: &[u8] = include_bytes!("../data/tally-no-device.json");

#[tokio::test]
async fn devices() {
    let sink = Sink::default();
    let backends = Backends::defaults().await.messaging(sink.clone());
    let guest = precompiled(TALLY_CONNECTOR_GUEST).expect("guest precompiles");
    let runtime = Deployment::new()
        .guest("tally-connector", guest)
        .boot(backends, |deployment| {
            deployment.host::<WasiHttp>()?;
            deployment.host::<WasiMessaging>()?;
            deployment.host::<WasiOtel>()?;
            Ok(())
        })
        .await
        .expect("runtime boots");

    for payload in [TALLY_MESSAGE, TALLY_NO_DEVICE] {
        let request = Request::post("/api/apc")
            .header(CONTENT_TYPE, "application/json")
            .body(Bytes::from_static(payload))
            .expect("request");
        let response = send(&runtime, request).await.expect("request served");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.body().as_ref(), br#""OK""#);
    }
    runtime.shutdown();

    // one record per reading, keyed by the device's site
    let sent = sink.sent();
    assert!(sent.iter().all(|message| message.topic == topic::TALLY_APC));
    let keys: Vec<_> = sent
        .iter()
        .map(|message| message.metadata.as_ref().and_then(|metadata| metadata.inner.get("key")))
        .map(|key| key.map(String::as_str))
        .collect();
    assert_eq!(keys, [Some("EM1210"), Some("undefined")]);

    // each payload carries the reading's fields under their wire names
    let payloads: Vec<Value> = sent
        .iter()
        .map(|message| serde_json::from_slice(&message.payload).expect("payload is JSON"))
        .collect();
    assert_eq!(
        payloads[0]["device"],
        json!({ "operator": "ACME", "site": "EM1210", "model": "CARM1", "serial": "2312-00125" })
    );
    assert!(payloads[1]["device"].is_null());
    for payload in &payloads {
        assert_reading(payload);
    }
}

fn assert_reading(payload: &Value) {
    for (field, expected) in [
        ("tly_vers", json!("1.0")),
        ("tly_type", json!("td")),
        ("driving", json!(true)),
        ("atstop", json!(false)),
        ("operational", json!(true)),
        ("distance_laststop", json!(10213)),
        ("distance_start", json!(40301)),
        ("departure_utc", json!("1764194985")),
        ("arrival_utc", json!("1764194937")),
        ("speed", json!(0)),
        ("trigger", json!("station_left_summary")),
        ("wpt", json!({ "sat": "12", "lat": "-36.909900", "lon": "174.815500", "speed": 0 })),
        ("clock", json!({ "utc": "1764194985", "tz": "NZST-12NZDT,M9.5.0,M4.1.0/3" })),
        ("pis", json!({ "line": "54", "stop": "12343" })),
    ] {
        assert_eq!(payload[field], expected, "{field}");
    }

    let doors = payload["doors"].as_array().expect("doors");
    assert_eq!(doors.len(), 12);
    let door = &doors[4];
    assert_eq!(door["name"], "TD1");
    assert_eq!(door["in"], 1);
    assert_eq!(door["out"], 1);
    assert_eq!(door["art"], 1);
    assert_eq!(door["st"], "open");
}
