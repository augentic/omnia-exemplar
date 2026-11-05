//! The gtfs-adapter test provider and the exact upstream URLs the handlers
//! call, so each test seeds the request it expects rather than a prefix.

use bytes::Bytes;
use chrono::{DateTime, Utc};
use common::config;
use gtfs_adapter::block_mgt::{API_IDENTITY, BLOCK_MGT_URL};
use gtfs_adapter::fleet::{FLEET_URL, Identifier};
use gtfs_adapter::trip::TRIP_MANAGEMENT_URL;
use http::Response;
use omnia_test::guest::{FixedIdentity, MapConfig, Provider};

pub const FLEET_BASE: &str = "http://fleet.test";
pub const BLOCK_MGT_BASE: &str = "http://block-mgt.test";
pub const TRIP_MANAGEMENT_BASE: &str = "http://trip-mgt.test";

/// A provider seeded with the configuration keys the handlers read.
pub fn provider() -> Provider {
    Provider::default()
        .config(MapConfig::default().with([
            (config::ENV, "dev"),
            (BLOCK_MGT_URL, BLOCK_MGT_BASE),
            (FLEET_URL, FLEET_BASE),
            (TRIP_MANAGEMENT_URL, TRIP_MANAGEMENT_BASE),
            (API_IDENTITY, "test-identity"),
        ]))
        .identity(FixedIdentity::new("mock_access_token"))
}

/// The Fleet API lookup `gtfs_adapter::fleet::vehicle` makes for `vehicle_id`.
pub fn fleet_query(vehicle_id: &str) -> String {
    let identifier: Identifier = vehicle_id.parse().expect("identifier parse is infallible");
    format!("{FLEET_BASE}/vehicles?{}", identifier.to_query())
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
