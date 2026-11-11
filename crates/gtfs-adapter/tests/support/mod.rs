//! The gtfs-adapter test provider and the exact upstream URLs the handlers
//! call, so each test seeds the request it expects rather than a prefix.

use bytes::Bytes;
use chrono::{DateTime, Utc};
use http::Response;
use omnia_sdk::api::{Client, Router};
use omnia_test::guest::{FixedIdentity, MapConfig, Provider};
use serde_json::{Value, json};

pub const FLEET_BASE: &str = "http://fleet.test";
pub const BLOCK_MGT_BASE: &str = "http://block-mgt.test";
pub const TRIP_MANAGEMENT_BASE: &str = "http://trip-mgt.test";

/// A provider seeded with the configuration keys the handlers read.
pub fn provider() -> Provider {
    Provider::default()
        .config(MapConfig::default().with([
            ("BLOCK_MGT_URL", BLOCK_MGT_BASE),
            ("FLEET_URL", FLEET_BASE),
            ("TRIP_MANAGEMENT_URL", TRIP_MANAGEMENT_BASE),
            ("API_IDENTITY", "test-identity"),
        ]))
        .identity(FixedIdentity::new("mock_access_token"))
}

/// The stem's router over `provider`.
pub fn router(provider: &Provider) -> Router<Provider> {
    gtfs_adapter::handlers().with_state(Client::new(provider.clone()))
}

/// The Fleet API lookup `gtfs_adapter::fleet::vehicle` makes for `vehicle_id`.
pub fn fleet_query(vehicle_id: &str) -> String {
    let query = match vehicle_id {
        "EMP484" => "label=EMP%20%20%20%20%20%20%20%20484",
        "EM633" => "label=EM%20%20%20%20%20%20%20%20%20633",
        other => panic!("unexpected test vehicle_id: {other}"),
    };
    format!("{FLEET_BASE}/vehicles?{query}")
}

/// The Block Management lookup `cached_allocation` makes for a fleet id at a
/// message timestamp.
pub fn allocation_query(fleet_id: &str, at: DateTime<Utc>) -> String {
    format!(
        "{BLOCK_MGT_BASE}/allocations/vehicles/{fleet_id}?currentTrip=true&siblings=true&nowUnixTimeSeconds={}",
        at.timestamp()
    )
}

/// The Trip Management instances endpoint.
pub fn trip_instances() -> String {
    format!("{TRIP_MANAGEMENT_BASE}/tripinstances")
}

/// A body matcher for the Trip Management lookup of `trip_id` on `service_date`.
pub fn trip_query(
    trip_id: &str, service_date: &str,
) -> impl Fn(&[u8]) -> bool + Send + Sync + 'static {
    let expected = json!({ "tripIds": [trip_id], "serviceDate": service_date });
    move |body: &[u8]| serde_json::from_slice::<Value>(body).is_ok_and(|value| value == expected)
}

/// A trip instance as returned by the Trip Management API.
pub fn trip_instance(trip_id: &str) -> Value {
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

/// A serial-data message signing `vehicle_id` on to `trip_id` at `at`.
pub fn serial_data(vehicle_id: &str, trip_id: &str, at: DateTime<Utc>) -> String {
    json!({
        "eventType": "serialData",
        "remoteData": { "externalId": vehicle_id },
        "messageData": { "timestamp": at.to_rfc3339() },
        "serialData": {
            "decodedSerialData": { "tripId": trip_id, "tripNumber": trip_id, "lineId": "L1" },
        },
    })
    .to_string()
}

/// A `200` carrying `body`.
pub fn ok(body: impl Into<Bytes>) -> Response<Bytes> {
    Response::new(body.into())
}
