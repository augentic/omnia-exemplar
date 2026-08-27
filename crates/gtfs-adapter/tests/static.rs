//! Static fixture tests for the gtfs-adapter handlers.
//!
//! Fixtures under `data/` are captured Kafka records from a live system;
//! each test takes a record's `value` and invokes the handler natively
//! against the in-memory [`provider::MockProvider`].

mod provider;

use gtfs_adapter::{MotionMessage, PassengerCountMessage, TrainAvlMessage, VehicleInfoRequest};
use omnia_guest::api::{Client, Metadata};
use serde_json::{Value, json};

use self::provider::MockProvider;

const OWNER: &str = "acme";

/// Extract the `value` payload of the fixture record at `index`.
fn fixture_value(raw: &[u8], index: usize) -> Value {
    let records: Value = serde_json::from_slice(raw).expect("should parse fixture");
    records[index]["value"].clone()
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
async fn motion_location_publishes_vehicle_position() {
    let provider = MockProvider::new();
    provider.respond_with("http://fleet.test", include_bytes!("../data/fleet-query.json").to_vec());
    provider.respond_with("http://block-mgt.test", b"null".to_vec());

    let value = fixture_value(include_bytes!("../data/realtime-pulse-to-motion.v1.json"), 0);
    let message: MotionMessage = serde_json::from_value(value).expect("should deserialize");

    Client::new(OWNER, provider.clone())
        .call(message, &Metadata::default())
        .await
        .expect("should succeed");

    let published = provider.published();
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
async fn motion_without_gps_publishes_dead_reckoning() {
    let provider = MockProvider::new();
    provider.respond_with("http://fleet.test", include_bytes!("../data/fleet-query.json").to_vec());
    // allocation matches the stored trip, so the current trip is retained
    provider.respond_with(
        "http://block-mgt.test",
        serde_json::to_vec(&json!({
            "tripId": "TRIP-7",
            "startTime": "11:30:00",
            "serviceDate": "20251120",
            "vehicleIds": ["59144"],
            "error": false,
        }))
        .expect("should serialize"),
    );
    provider.seed_state(
        "motionGtfs:trip:vehicle:59144",
        serde_json::to_vec(&trip_instance("TRIP-7")).expect("should serialize"),
    );

    let message: MotionMessage = serde_json::from_value(json!({
        "eventType": "location",
        "remoteData": { "externalId": "EMP484" },
        "messageData": { "timestamp": "2025-11-19T22:38:16.559Z" },
        "locationData": { "gpsAccuracy": 0.0 },
        "eventData": { "odometer": 862_094_108.0 },
    }))
    .expect("should deserialize");

    Client::new(OWNER, provider.clone())
        .call(message, &Metadata::default())
        .await
        .expect("should succeed");

    let published = provider.published();
    assert_eq!(published.len(), 1);

    let (topic, record) = &published[0];
    assert_eq!(topic, "dev-realtime-dead-reckoning.v1");

    let dr: Value = serde_json::from_slice(&record.payload).expect("should parse payload");
    assert_eq!(dr["trip"]["tripId"], "TRIP-7");
    assert_eq!(dr["vehicle"]["id"], "59144");
    assert_eq!(dr["position"]["odometer"], json!(862_094_108.0));
}

#[tokio::test]
async fn motion_unknown_event_type_is_ignored() {
    let provider = MockProvider::new();

    let message: MotionMessage = serde_json::from_value(json!({
        "eventType": "somethingElse",
        "remoteData": { "externalId": "EMP484" },
        "messageData": { "timestamp": "2025-11-19T22:38:16.559Z" },
    }))
    .expect("should deserialize");

    Client::new(OWNER, provider.clone())
        .call(message, &Metadata::default())
        .await
        .expect("should succeed");

    assert!(provider.published().is_empty());
}

#[tokio::test]
async fn train_avl_skips_non_motion_tag() {
    let provider = MockProvider::new();
    // the fixture fleet record is tagged "NOVA", so the filter drops it
    provider.respond_with("http://fleet.test", include_bytes!("../data/fleet-query.json").to_vec());

    let value = fixture_value(include_bytes!("../data/realtime-train-avl.v1.json"), 0);
    let message: TrainAvlMessage = serde_json::from_value(value).expect("should deserialize");

    Client::new(OWNER, provider.clone())
        .call(message, &Metadata::default())
        .await
        .expect("should succeed");

    assert!(provider.published().is_empty());
}

#[tokio::test]
async fn train_avl_processes_motion_tag() {
    let provider = MockProvider::new();

    let mut fleet: Value = serde_json::from_slice(include_bytes!("../data/fleet-query.json"))
        .expect("should parse fixture");
    fleet[0]["tag"] = "motion".into();
    provider.respond_with("http://fleet.test", serde_json::to_vec(&fleet).expect("serialize"));
    provider.respond_with("http://block-mgt.test", b"null".to_vec());

    let value = fixture_value(include_bytes!("../data/realtime-train-avl.v1.json"), 0);
    let message: TrainAvlMessage = serde_json::from_value(value).expect("should deserialize");

    Client::new(OWNER, provider.clone())
        .call(message, &Metadata::default())
        .await
        .expect("should succeed");

    let published = provider.published();
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].0, "dev-realtime-gtfs-vp.v1");
}

#[tokio::test]
async fn passenger_count_stores_occupancy_status() {
    let provider = MockProvider::new();

    let value = fixture_value(include_bytes!("../data/realtime-passenger-count.v1.json"), 0);
    let message: PassengerCountMessage = serde_json::from_value(value).expect("should deserialize");

    Client::new(OWNER, provider.clone())
        .call(message, &Metadata::default())
        .await
        .expect("should succeed");

    let key = "motionGtfs:occupancyStatus:32161:1347-05004-41400-2-89c4020e:20251120:11:30:00";
    let stored = provider.state(key).expect("occupancy status should be stored");
    assert_eq!(stored, b"\"FEW_SEATS_AVAILABLE\"");
}

#[tokio::test]
async fn passenger_count_clears_occupancy_status() {
    let provider = MockProvider::new();
    let key = "motionGtfs:occupancyStatus:32161:1347-05004-41400-2-89c4020e:20251120:11:30:00";
    provider.seed_state(key, b"\"FEW_SEATS_AVAILABLE\"".to_vec());

    let mut value = fixture_value(include_bytes!("../data/realtime-passenger-count.v1.json"), 0);
    value.as_object_mut().expect("object").remove("occupancyStatus");
    let message: PassengerCountMessage = serde_json::from_value(value).expect("should deserialize");

    Client::new(OWNER, provider.clone())
        .call(message, &Metadata::default())
        .await
        .expect("should succeed");

    assert!(provider.state(key).is_none(), "occupancy status should be cleared");
}

#[tokio::test]
async fn serial_data_sign_on_updates_trip_state() {
    let provider = MockProvider::new();
    let now = chrono::Utc::now();
    let today = now.format("%Y%m%d").to_string();

    // the vehicle is currently on a different trip
    let mut previous = trip_instance("TRIP-0");
    previous["serviceDate"] = today.clone().into();
    provider.seed_state(
        "motionGtfs:trip:vehicle:EM580",
        serde_json::to_vec(&previous).expect("serialize"),
    );

    // trip management resolves the keyed-in trip
    let mut instance = trip_instance("TRIP-1");
    instance["serviceDate"] = today.into();
    provider.respond_with(
        "http://trip-mgt.test",
        serde_json::to_vec(&json!([instance])).expect("serialize"),
    );

    let message: MotionMessage = serde_json::from_value(json!({
        "eventType": "serialData",
        "remoteData": { "externalId": "EM580" },
        "messageData": { "timestamp": now.to_rfc3339() },
        "serialData": {
            "decodedSerialData": { "tripId": "TRIP-1", "tripNumber": "TRIP-1", "lineId": "L1" },
        },
    }))
    .expect("should deserialize");

    Client::new(OWNER, provider.clone())
        .call(message, &Metadata::default())
        .await
        .expect("should succeed");

    let trip = provider.state("motionGtfs:trip:vehicle:EM580").expect("trip should be stored");
    let trip: Value = serde_json::from_slice(&trip).expect("should parse trip");
    assert_eq!(trip["tripId"], "TRIP-1");

    assert!(provider.state("motionGtfs:vehicle:signOn:EM580").is_some());
    assert!(provider.state("motionGtfs:serialTimestamp:EM580").is_some());
}

#[tokio::test]
async fn serial_data_rejects_future_dated_message() {
    let provider = MockProvider::new();
    let future = chrono::Utc::now() + chrono::Duration::seconds(3_600);

    let message: MotionMessage = serde_json::from_value(json!({
        "eventType": "serialData",
        "remoteData": { "externalId": "EM580" },
        "messageData": { "timestamp": future.to_rfc3339() },
        "serialData": {
            "decodedSerialData": { "tripId": "TRIP-1", "tripNumber": "TRIP-1", "lineId": "L1" },
        },
    }))
    .expect("should deserialize");

    let error = Client::new(OWNER, provider.clone())
        .call(message, &Metadata::default())
        .await
        .expect_err("should reject future-dated message");
    assert!(error.to_string().contains("future-dated"));
}

#[tokio::test]
async fn vehicle_info_assembles_state_and_fleet_data() {
    let provider = MockProvider::new();
    provider.respond_with("http://fleet.test", include_bytes!("../data/fleet-query.json").to_vec());
    provider.seed_state(
        "motionGtfs:trip:vehicle:EMP484",
        serde_json::to_vec(&trip_instance("TRIP-7")).expect("serialize"),
    );
    provider.seed_state("motionGtfs:vehicle:signOn:EMP484", b"1763592000".to_vec());

    let request = VehicleInfoRequest {
        vehicle_id: "EMP484".to_string(),
    };
    let reply = Client::new(OWNER, provider.clone())
        .call(request, &Metadata::default())
        .await
        .expect("should succeed");

    assert_eq!(reply.pid, 0);
    assert_eq!(reply.vehicle_id, "EMP484");
    assert_eq!(reply.sign_on_time.as_deref(), Some("1763592000"));
    assert_eq!(reply.trip_info.expect("trip info").trip_id, "TRIP-7");
    assert_eq!(reply.fleet_info.expect("fleet info").id, "59144");
}
