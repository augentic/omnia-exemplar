//! Static tests for the Tally APC connector driven through its router.

use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use common::topic;
use omnia_sdk::api::Client;
use omnia_test::guest::{Provider, send};
use serde_json::{Value, json};

const TALLY_MESSAGE: &[u8] = include_bytes!("../data/tally-message.json");
const TALLY_NO_DEVICE: &[u8] = include_bytes!("../data/tally-no-device.json");

async fn forward(provider: &Provider, payload: &'static [u8]) {
    let request = Request::post("/api/apc")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(payload))
        .expect("request");
    let router = tally_connector::handlers().with_state(Client::new(provider.clone()));
    let response = send(&router, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.body().as_ref(), br#""OK""#);
}

#[tokio::test]
async fn devices() {
    let provider = Provider::default();

    forward(&provider, TALLY_MESSAGE).await;
    forward(&provider, TALLY_NO_DEVICE).await;

    // one record per reading, keyed by the device's site
    let published = provider.publish.sent();
    assert!(published.iter().all(|(name, _)| name == topic::TALLY_APC));
    let keys: Vec<_> =
        published.iter().map(|(_, record)| record.headers.get("key").map(String::as_str)).collect();
    assert_eq!(keys, [Some("EM1210"), Some("undefined")]);

    // each payload carries the reading's fields under their wire names
    let payloads: Vec<Value> = published
        .iter()
        .map(|(_, record)| serde_json::from_slice(&record.payload).expect("payload is JSON"))
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
