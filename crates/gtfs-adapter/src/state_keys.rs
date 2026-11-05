//! State store key conventions.
//!
//! All gtfs-adapter state lives under the `motionGtfs:` prefix, namespaced by
//! purpose and then by identifier:
//!
//! | Key | Value |
//! | --- | --- |
//! | `motionGtfs:trip:vehicle:{vehicle_id}` | the vehicle's current [`TripInstance`](crate::trip::TripInstance) |
//! | `motionGtfs:vehicle:signOn:{vehicle_id}` | the Unix timestamp of the vehicle's sign-on |
//! | `motionGtfs:serialTimestamp:{vehicle_id}` | the newest serial data timestamp seen for the vehicle |
//! | `motionGtfs:occupancyStatus:{vehicle_id}:{trip_id}:{start_date}:{start_time}` | the GTFS-realtime occupancy status |
//!
//! God-mode overrides live under a separate `god_mode:` prefix (see
//! `god_mode`) so operational overrides are never confused with
//! pipeline state.

#[must_use]
pub fn trip(vehicle_id: &str) -> String {
    format!("motionGtfs:trip:vehicle:{vehicle_id}")
}

#[must_use]
pub fn sign_on(vehicle_id: &str) -> String {
    format!("motionGtfs:vehicle:signOn:{vehicle_id}")
}

#[must_use]
pub fn serial_timestamp(vehicle_id: &str) -> String {
    format!("motionGtfs:serialTimestamp:{vehicle_id}")
}

#[must_use]
pub fn occupancy_status(
    vehicle_id: &str, trip_id: &str, start_date: &str, start_time: &str,
) -> String {
    format!("motionGtfs:occupancyStatus:{vehicle_id}:{trip_id}:{start_date}:{start_time}")
}
