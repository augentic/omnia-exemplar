//! The gtfs-adapter test provider and the exact upstream URLs the handlers
//! call, so each test seeds the request it expects rather than a prefix.

use bytes::Bytes;
use chrono::{DateTime, Utc};
use http::Response;
use omnia_test::guest::{FixedIdentity, MapConfig, Provider};

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

/// A `200` carrying `body`.
pub fn ok(body: impl Into<Bytes>) -> Response<Bytes> {
    Response::new(body.into())
}
