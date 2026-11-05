//! Static fixture tests for the gtfs-adapter routes and topics.
//!
//! Fixtures under `data/` are captured Kafka records from a live system;
//! each test takes a record's `value` and drives the stem's `router()` or
//! `topics(..)` natively against `omnia_test` doubles seeded with the
//! upstream answers.

mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use chrono::{DateTime, Utc};
use common::{config, topic};
use gtfs_adapter::VehicleInfoReply;
use omnia_sdk::api::Client;
use omnia_sdk::api::messaging::{Delivery, DeliveryError, Router};
use omnia_test::guest::{MatchedHttp, Provider};
use serde_json::{Value, json};
use tower::ServiceExt as _;

use self::support::{allocation_query, fleet_query, ok, provider, trip_instances};

const OWNER: &str = "acme";
const ENV: &str = "dev";
#[expect(
    clippy::disallowed_macros,
    reason = "integration test loads captured fixture JSON from disk"
)]
const FLEET_QUERY: &[u8] = include_bytes!("../data/fleet-query.json");

fn topics(provider: &Provider) -> Router<Provider> {
    gtfs_adapter::topics(Router::new(Client::new(OWNER, provider.clone())), ENV)
}

fn delivery(suffix: &str, payload: impl Into<Vec<u8>>) -> Delivery {
    Delivery {
        topic: Some(config::topic_for(ENV, suffix)),
        payload: payload.into(),
        ..Delivery::default()
    }
}

async fn send(provider: Provider, request: Request<Body>) -> (StatusCode, Value) {
    let router = gtfs_adapter::router().with_state(Client::new(OWNER, provider));
    let response = router.oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

/// Extract the `value` payload of the fixture record at `index`.
fn fixture_value(raw: &[u8], index: usize) -> Value {
    let records: Value = serde_json::from_slice(raw).expect("should parse fixture");
    records[index]["value"].clone()
}

/// The `messageData.timestamp` of a Motion-shaped message, as the handlers
/// read it for the allocation lookup.
fn message_timestamp(value: &Value) -> DateTime<Utc> {
    value["messageData"]["timestamp"]
        .as_str()
        .expect("timestamp")
        .parse()
        .expect("RFC 3339 timestamp")
}

/// A trip instance as returned by the Trip Management API.
fn trip_instance(trip_id: &str) -> Value {
    json!({
        "tripId": trip_id,
        "routeId": "R1",
        "serviceDate": "20251120",
        "startTime": "11:30:00",
        "endTime": "12:30:00",
        "directionId": null,
        "isAddedTrip": false,
    })
}

#[tokio::test]
async fn motion_location() {
    #[expect(
        clippy::disallowed_macros,
        reason = "integration test loads captured fixture JSON from disk"
    )]
    let value = fixture_value(include_bytes!("../data/realtime-pulse-to-motion.v1.json"), 0);
    let provider = provider().http(
        MatchedHttp::default().on(http::Method::GET, fleet_query("EMP484"), ok(FLEET_QUERY)).on(
            http::Method::GET,
            allocation_query("59144", message_timestamp(&value)),
            ok("null"),
        ),
    );

    topics(&provider)
        .handle(delivery(topic::PULSE_TO_MOTION, value.to_string()))
        .await
        .expect("should succeed");

    let published = provider.publish.sent();
    assert_eq!(published.len(), 1);

    let (topic, record) = &published[0];
    assert_eq!(topic, "dev-realtime-gtfs-vp.v1");
    // the key is the fleet identifier resolved from the message's external id
    assert_eq!(record.headers.get("key").map(String::as_str), Some("59144"));

    let entity: Value = serde_json::from_slice(&record.payload).expect("should parse payload");
    assert_eq!(entity["id"], "59144");
    assert_eq!(entity["vehicle"]["position"]["latitude"], json!(-36.844_48));
}

#[tokio::test]
async fn motion_without_gps() {
    let value = json!({
        "eventType": "location",
        "remoteData": { "externalId": "EMP484" },
        "messageData": { "timestamp": "2025-11-19T22:38:16.559Z" },
        "locationData": { "gpsAccuracy": 0.0 },
        "eventData": { "odometer": 862_094_108.0 },
    });
    // allocation matches the stored trip, so the current trip is retained
    let allocation = json!({
        "tripId": "TRIP-7",
        "startTime": "11:30:00",
        "serviceDate": "20251120",
        "vehicleIds": ["59144"],
        "error": false,
    });
    let provider = provider().http(
        MatchedHttp::default().on(http::Method::GET, fleet_query("EMP484"), ok(FLEET_QUERY)).on(
            http::Method::GET,
            allocation_query("59144", message_timestamp(&value)),
            ok(allocation.to_string()),
        ),
    );
    provider.storage.insert_state(
        "motionGtfs:trip:vehicle:59144",
        &serde_json::to_vec(&trip_instance("TRIP-7")).expect("should serialize"),
    );

    topics(&provider)
        .handle(delivery(topic::PULSE_TO_MOTION, value.to_string()))
        .await
        .expect("should succeed");

    let published = provider.publish.sent();
    assert_eq!(published.len(), 1);

    let (topic, record) = &published[0];
    assert_eq!(topic, "dev-realtime-dead-reckoning.v1");

    let dr: Value = serde_json::from_slice(&record.payload).expect("should parse payload");
    assert_eq!(dr["trip"]["tripId"], "TRIP-7");
    assert_eq!(dr["vehicle"]["id"], "59144");
    assert_eq!(dr["position"]["odometer"], json!(862_094_108.0));
}

#[tokio::test]
async fn motion_unknown_event() {
    let provider = provider();
    let value = json!({
        "eventType": "somethingElse",
        "remoteData": { "externalId": "EMP484" },
        "messageData": { "timestamp": "2025-11-19T22:38:16.559Z" },
    });

    topics(&provider)
        .handle(delivery(topic::PULSE_TO_MOTION, value.to_string()))
        .await
        .expect("should succeed");

    assert!(provider.publish.sent().is_empty());
    assert!(provider.http.requests().is_empty());
}

#[tokio::test]
async fn train_avl_non_motion() {
    // the fixture fleet record is tagged "NOVA", so the filter drops it
    let provider = provider().http(MatchedHttp::default().on(
        http::Method::GET,
        fleet_query("EM633"),
        ok(FLEET_QUERY),
    ));
    #[expect(
        clippy::disallowed_macros,
        reason = "integration test loads captured fixture JSON from disk"
    )]
    let value = fixture_value(include_bytes!("../data/realtime-train-avl.v1.json"), 0);

    topics(&provider)
        .handle(delivery(topic::TRAIN_AVL, value.to_string()))
        .await
        .expect("should succeed");

    assert!(provider.publish.sent().is_empty());
}

#[tokio::test]
async fn train_avl_motion() {
    let mut fleet: Value = serde_json::from_slice(FLEET_QUERY).expect("should parse fixture");
    fleet[0]["tag"] = "motion".into();

    #[expect(
        clippy::disallowed_macros,
        reason = "integration test loads captured fixture JSON from disk"
    )]
    let value = fixture_value(include_bytes!("../data/realtime-train-avl.v1.json"), 0);
    let provider = provider().http(
        MatchedHttp::default()
            .on(http::Method::GET, fleet_query("EM633"), ok(fleet.to_string()))
            .on(
                http::Method::GET,
                allocation_query("59144", message_timestamp(&value)),
                ok("null"),
            ),
    );

    topics(&provider)
        .handle(delivery(topic::TRAIN_AVL, value.to_string()))
        .await
        .expect("should succeed");

    let published = provider.publish.sent();
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].0, "dev-realtime-gtfs-vp.v1");
}

/// A passenger count with an occupancy status, then the same count without
/// one: the status is stored under the trip key, then cleared from it.
#[tokio::test]
async fn passenger_count_occupancy() {
    let provider = provider();
    let key = "motionGtfs:occupancyStatus:32161:1347-05004-41400-2-89c4020e:20251120:11:30:00";

    #[expect(
        clippy::disallowed_macros,
        reason = "integration test loads captured fixture JSON from disk"
    )]
    let value = fixture_value(include_bytes!("../data/realtime-passenger-count.v1.json"), 0);

    topics(&provider)
        .handle(delivery(topic::PASSENGER_COUNT, value.to_string()))
        .await
        .expect("should succeed");

    let stored = provider.storage.state(key).expect("occupancy status should be stored");
    assert_eq!(stored, b"\"FEW_SEATS_AVAILABLE\"");

    // the same count without an occupancy status clears what was stored
    let mut value = value;
    value.as_object_mut().expect("object").remove("occupancyStatus");

    topics(&provider)
        .handle(delivery(topic::PASSENGER_COUNT, value.to_string()))
        .await
        .expect("should succeed");

    assert!(provider.storage.state(key).is_none(), "occupancy status should be cleared");
}

#[tokio::test]
async fn serial_data_sign_on() {
    let now = chrono::Utc::now();
    let today = now.format("%Y%m%d").to_string();

    // trip management resolves the keyed-in trip
    let mut instance = trip_instance("TRIP-1");
    instance["serviceDate"] = today.clone().into();
    let provider = provider().http(MatchedHttp::default().on(
        http::Method::POST,
        trip_instances(),
        ok(json!([instance]).to_string()),
    ));

    // the vehicle is currently on a different trip
    let mut previous = trip_instance("TRIP-0");
    previous["serviceDate"] = today.into();
    provider.storage.insert_state(
        "motionGtfs:trip:vehicle:EM580",
        &serde_json::to_vec(&previous).expect("serialize"),
    );

    let value = json!({
        "eventType": "serialData",
        "remoteData": { "externalId": "EM580" },
        "messageData": { "timestamp": now.to_rfc3339() },
        "serialData": {
            "decodedSerialData": { "tripId": "TRIP-1", "tripNumber": "TRIP-1", "lineId": "L1" },
        },
    });

    topics(&provider)
        .handle(delivery(topic::PULSE_TO_MOTION, value.to_string()))
        .await
        .expect("should succeed");

    let trip = provider.storage.state("motionGtfs:trip:vehicle:EM580").expect("trip stored");
    let trip: Value = serde_json::from_slice(&trip).expect("should parse trip");
    assert_eq!(trip["tripId"], "TRIP-1");

    assert!(provider.storage.state("motionGtfs:vehicle:signOn:EM580").is_some());
    assert!(provider.storage.state("motionGtfs:serialTimestamp:EM580").is_some());
}

#[tokio::test]
async fn serial_data_future() {
    let provider = provider();
    let future = chrono::Utc::now() + chrono::Duration::seconds(3_600);

    let value = json!({
        "eventType": "serialData",
        "remoteData": { "externalId": "EM580" },
        "messageData": { "timestamp": future.to_rfc3339() },
        "serialData": {
            "decodedSerialData": { "tripId": "TRIP-1", "tripNumber": "TRIP-1", "lineId": "L1" },
        },
    });

    let error = topics(&provider)
        .handle(delivery(topic::PULSE_TO_MOTION, value.to_string()))
        .await
        .expect_err("should reject future-dated message");
    assert!(error.to_string().contains("bad_time"));
    assert!(error.to_string().contains("future-dated"));
}

#[tokio::test]
async fn serial_data_outdated() {
    let provider = provider();
    let now = chrono::Utc::now();
    provider.storage.insert_state(
        "motionGtfs:serialTimestamp:EM580",
        &serde_json::to_vec(&now.timestamp()).expect("serialize"),
    );

    let older = now - chrono::Duration::seconds(60);
    let value = json!({
        "eventType": "serialData",
        "remoteData": { "externalId": "EM580" },
        "messageData": { "timestamp": older.to_rfc3339() },
        "serialData": {
            "decodedSerialData": { "tripId": "TRIP-1", "tripNumber": "TRIP-1", "lineId": "L1" },
        },
    });

    let error = topics(&provider)
        .handle(delivery(topic::PULSE_TO_MOTION, value.to_string()))
        .await
        .expect_err("should reject outdated message");
    assert_eq!(
        error,
        DeliveryError::Rejected(
            "code: bad_time, description: outdated serial data message".to_string()
        )
    );
}

#[tokio::test]
async fn vehicle_info_state() {
    let provider = provider().http(MatchedHttp::default().on(
        http::Method::GET,
        fleet_query("EMP484"),
        ok(FLEET_QUERY),
    ));
    provider.storage.insert_state(
        "motionGtfs:trip:vehicle:EMP484",
        &serde_json::to_vec(&trip_instance("TRIP-7")).expect("serialize"),
    );
    provider.storage.insert_state("motionGtfs:vehicle:signOn:EMP484", b"1763592000");

    let request = Request::get("/info/EMP484").body(Body::empty()).expect("request");
    let (status, body) = send(provider, request).await;

    assert_eq!(status, StatusCode::OK);
    let reply: VehicleInfoReply = serde_json::from_value(body).expect("reply");
    assert_eq!(reply.pid, 0);
    assert_eq!(reply.vehicle_id, "EMP484");
    assert_eq!(reply.sign_on_time.as_deref(), Some("1763592000"));
    assert_eq!(reply.trip_info.expect("trip info").trip_id, "TRIP-7");
    assert_eq!(reply.fleet_info.expect("fleet info").id, "59144");
}

#[tokio::test]
async fn other_environment() {
    let provider = provider();
    let topic = config::topic_for("prod", topic::PASSENGER_COUNT);
    let delivery = Delivery {
        topic: Some(topic.clone()),
        payload: b"{}".to_vec(),
        ..Delivery::default()
    };

    let error = topics(&provider).handle(delivery).await.expect_err("wrong env");

    assert_eq!(error, DeliveryError::UnhandledTopic(topic));
    assert!(provider.storage.is_empty());
}
