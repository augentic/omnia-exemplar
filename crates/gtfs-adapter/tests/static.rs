//! Static fixture tests for the gtfs-adapter routes and topics.
//!
//! Fixtures under `data/` are captured Kafka records from a live system;
//! each test takes a record's `value` and drives the stem's `routes()` or
//! `topics()` natively against `omnia_test` doubles seeded with the
//! upstream answers.

mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{DateTime, Utc};
use common::topic;
use gtfs_adapter::vehicle_info::VehicleInfoReply;
use omnia_sdk::api::messaging::DeliveryError;
use omnia_test::guest::{MatchedHttp, deliver, send};
use serde_json::{Value, json};

use self::support::{
    allocation_query, fleet_query, ok, provider, router, serial_data, trip_instance,
    trip_instances, trip_query,
};

const FLEET_QUERY: &[u8] = include_bytes!("../data/fleet-query.json");
const PULSE_TO_MOTION: &[u8] = include_bytes!("../data/realtime-pulse-to-motion.v1.json");
const TRAIN_AVL: &[u8] = include_bytes!("../data/realtime-train-avl.v1.json");
const PASSENGER_COUNT: &[u8] = include_bytes!("../data/realtime-passenger-count.v1.json");

fn fixture_value(raw: &[u8], index: usize) -> Value {
    let records: Value = serde_json::from_slice(raw).expect("should parse fixture");
    records[index]["value"].clone()
}

// The handlers key the allocation lookup on this timestamp.
fn message_timestamp(value: &Value) -> DateTime<Utc> {
    value["messageData"]["timestamp"]
        .as_str()
        .expect("timestamp")
        .parse()
        .expect("RFC 3339 timestamp")
}

#[tokio::test]
async fn motion_location() {
    let value = fixture_value(PULSE_TO_MOTION, 0);
    let provider = provider().http(
        MatchedHttp::default().on(http::Method::GET, fleet_query("EMP484"), ok(FLEET_QUERY)).on(
            http::Method::GET,
            allocation_query("59144", message_timestamp(&value)),
            ok("null"),
        ),
    );

    deliver(&router(&provider), topic::PULSE_TO_MOTION, value.to_string())
        .await
        .expect("should succeed");

    let published = provider.publisher.sent();
    assert_eq!(published.len(), 1);

    let (topic, record) = &published[0];
    assert_eq!(topic, topic::GTFS_VP);
    // the key is the fleet identifier resolved from the message's external id
    assert_eq!(record.key.as_deref(), Some("59144"));

    let entity: Value = serde_json::from_slice(&record.payload).expect("should parse payload");
    assert_eq!(entity["id"], "59144");
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
        "motionGtfs.trip.vehicle.59144",
        &serde_json::to_vec(&trip_instance("TRIP-7")).expect("should serialize"),
    );

    deliver(&router(&provider), topic::PULSE_TO_MOTION, value.to_string())
        .await
        .expect("should succeed");

    let published = provider.publisher.sent();
    assert_eq!(published.len(), 1);

    let (topic, record) = &published[0];
    assert_eq!(topic, topic::DEAD_RECKONING);

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

    deliver(&router(&provider), topic::PULSE_TO_MOTION, value.to_string())
        .await
        .expect("should succeed");

    assert_eq!(provider.publisher.sent(), []);
    assert!(provider.http.requests().is_empty());
}

#[tokio::test]
async fn train_avl() {
    let nova = fixture_value(TRAIN_AVL, 0);
    let motion = fixture_value(PULSE_TO_MOTION, 0);
    let mut motion_fleet: Value =
        serde_json::from_slice(FLEET_QUERY).expect("should parse fixture");
    motion_fleet[0]["tag"] = "motion".into();

    // EM633 is tagged "NOVA" and EMP484 "motion". Only EMP484's allocation is
    // scripted, so EM633 passing the filter panics on an unscripted request.
    let provider = provider().http(
        MatchedHttp::default()
            .on(http::Method::GET, fleet_query("EM633"), ok(FLEET_QUERY))
            .on(http::Method::GET, fleet_query("EMP484"), ok(motion_fleet.to_string()))
            .on(
                http::Method::GET,
                allocation_query("59144", message_timestamp(&motion)),
                ok("null"),
            ),
    );

    for value in [nova, motion] {
        deliver(&router(&provider), topic::TRAIN_AVL, value.to_string())
            .await
            .expect("should succeed");
    }

    assert_eq!(provider.publisher.sent().len(), 1);
}

#[tokio::test]
async fn pax_occupancy() {
    let provider = provider();
    let key = "motionGtfs.occupancyStatus.32161.1347-05004-41400-2-89c4020e.20251120.113000";

    let value = fixture_value(PASSENGER_COUNT, 0);

    deliver(&router(&provider), topic::PASSENGER_COUNT, value.to_string())
        .await
        .expect("should succeed");

    let stored = provider.storage.state(key).expect("occupancy status should be stored");
    assert_eq!(stored, b"\"FEW_SEATS_AVAILABLE\"");

    // the same count without an occupancy status clears what was stored
    let mut value = value;
    value.as_object_mut().expect("object").remove("occupancyStatus");

    deliver(&router(&provider), topic::PASSENGER_COUNT, value.to_string())
        .await
        .expect("should succeed");

    assert!(provider.storage.state(key).is_none(), "occupancy status should be cleared");
}

#[tokio::test]
async fn serial_data_sign_on() {
    // a fixed past instant after 04:00 keeps the lookup to one service date
    let signed_on: DateTime<Utc> = "2025-11-20T12:00:00Z".parse().expect("timestamp");

    // 36:05:00 is 12:05 on the next day, so TRIP-EXT ranks behind TRIP-1
    // only if an extended-hours start time rolls over to that day
    let mut instance = trip_instance("TRIP-1");
    instance["startTime"] = "12:30:00".into();
    let mut extended = trip_instance("TRIP-EXT");
    extended["startTime"] = "36:05:00".into();
    let provider = provider().http(MatchedHttp::default().on_matching(
        http::Method::POST,
        trip_instances(),
        trip_query("TRIP-1", "20251120"),
        ok(json!([extended, instance]).to_string()),
    ));

    // the vehicle is currently on a different trip
    provider.storage.insert_state(
        "motionGtfs.trip.vehicle.EM580",
        &serde_json::to_vec(&trip_instance("TRIP-0")).expect("serialize"),
    );

    // sign on
    let message = serial_data("EM580", "TRIP-1", signed_on);
    deliver(&router(&provider), topic::PULSE_TO_MOTION, message.clone())
        .await
        .expect("should succeed");

    let trip = provider.storage.state("motionGtfs.trip.vehicle.EM580").expect("trip stored");
    let trip: Value = serde_json::from_slice(&trip).expect("should parse trip");
    assert_eq!(trip["tripId"], "TRIP-1");
    assert_eq!(
        provider.storage.state("motionGtfs.vehicle.signOn.EM580"),
        Some(signed_on.timestamp().to_string().into_bytes())
    );

    // redelivery
    let error = deliver(&router(&provider), topic::PULSE_TO_MOTION, message)
        .await
        .expect_err("should reject outdated message");
    assert!(
        matches!(&error, DeliveryError::Failed { message, .. } if message.contains("outdated")),
        "{error}"
    );

    // the future check is against the wall clock
    let future = serial_data("EM580", "TRIP-1", Utc::now() + chrono::Duration::seconds(3_600));
    let error = deliver(&router(&provider), topic::PULSE_TO_MOTION, future)
        .await
        .expect_err("should reject future-dated message");
    assert!(
        matches!(&error, DeliveryError::Failed { message, .. } if message.contains("future-dated")),
        "{error}"
    );

    // neither rejection reaches Trip Management
    assert_eq!(provider.http.requests().len(), 1);
}

#[tokio::test]
async fn vehicle_info_state() {
    let provider = provider().http(MatchedHttp::default().on(
        http::Method::GET,
        fleet_query("EMP484"),
        ok(FLEET_QUERY),
    ));
    provider.storage.insert_state(
        "motionGtfs.trip.vehicle.EMP484",
        &serde_json::to_vec(&trip_instance("TRIP-7")).expect("serialize"),
    );
    provider.storage.insert_state("motionGtfs.vehicle.signOn.EMP484", b"1763592000");

    let request = Request::get("/info/EMP484").body(Body::empty()).expect("request");
    let response = send(&router(&provider), request).await;

    assert_eq!(response.status(), StatusCode::OK);
    let reply: VehicleInfoReply = serde_json::from_slice(response.body()).expect("reply");
    assert_eq!(reply.sign_on_time.as_deref(), Some("1763592000"));
    assert_eq!(reply.trip_info.expect("trip info").trip_id, "TRIP-7");
    assert_eq!(reply.fleet_info.expect("fleet info").id, "59144");
}

// a `:` cannot sit in a state key segment, so the id never reaches a key
#[tokio::test]
async fn non_portable_vehicle_id() {
    let provider = provider();

    let request = Request::get("/info/EMP:484").body(Body::empty()).expect("request");
    let response = send(&router(&provider), request).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: Value = serde_json::from_slice(response.body()).expect("error JSON");
    assert_eq!(body["error"], "bad_request");
    assert!(body["message"].as_str().is_some_and(|msg| msg.contains("EMP:484")), "{body}");
    assert!(provider.http.requests().is_empty());
}
