//! The gtfs-adapter test provider and the exact upstream URLs the handlers
//! call, so each test seeds the request it expects rather than a prefix.

#![allow(dead_code, reason = "shared by several test binaries")]

use acme_common::config;
use acme_common::fleet::Identifier;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use http::Response;
use omnia_test::guest::{FixedIdentity, MapConfig};

omnia_test::provider! {
    /// The union of the handlers' capability lists, as doubles.
    pub struct TestProvider: Config + HttpRequest + Identity + Publish + StateStore;
}

pub const FLEET_URL: &str = "http://fleet.test";
pub const BLOCK_MGT_URL: &str = "http://block-mgt.test";
pub const TRIP_MANAGEMENT_URL: &str = "http://trip-mgt.test";

/// A provider seeded with the configuration keys the handlers read.
pub fn provider() -> TestProvider {
    TestProvider::default()
        .config(MapConfig::default().with([
            (config::ENV, "dev"),
            (config::BLOCK_MGT_URL, BLOCK_MGT_URL),
            (config::FLEET_URL, FLEET_URL),
            (config::TRIP_MANAGEMENT_URL, TRIP_MANAGEMENT_URL),
            (config::STATIC_API_URL, "http://static.test"),
            (config::API_IDENTITY, "test-identity"),
        ]))
        .identity(FixedIdentity::new("mock_access_token"))
}

/// The Fleet API lookup `acme_common::fleet::vehicle` makes for `vehicle_id`.
pub fn fleet_query(vehicle_id: &str) -> String {
    let identifier: Identifier = vehicle_id.parse().expect("identifier parse is infallible");
    format!("{FLEET_URL}/vehicles?{}", identifier.to_query())
}

/// The Block Management lookup `cached_allocation` makes for a fleet id at a
/// message timestamp.
pub fn allocation_query(fleet_id: &str, at: DateTime<Utc>) -> String {
    format!(
        "{BLOCK_MGT_URL}/allocations/vehicles/{fleet_id}?currentTrip=true&siblings=true&nowUnixTimeSeconds={}",
        at.timestamp()
    )
}

/// The Trip Management instances endpoint.
pub fn trip_instances() -> String {
    format!("{TRIP_MANAGEMENT_URL}/tripinstances")
}

/// A `200` carrying `body`.
pub fn ok(body: impl Into<Bytes>) -> Response<Bytes> {
    Response::new(body.into())
}
