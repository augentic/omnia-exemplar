//! # Motion GTFS adapter
//!
//! Transforms Motion AVL telemetry into GTFS-realtime vehicle positions.

pub mod block_mgt;
pub mod fleet;
#[cfg(feature = "god-mode")]
pub mod god_mode;
pub mod location;
pub mod motion;
pub mod passenger_count;
pub mod serial_data;
#[cfg(feature = "god-mode")]
pub mod set_trip;
pub mod state_keys;
pub mod train_avl;
pub mod trip;
pub mod vehicle_info;

use common::Capabilities;
pub use motion::{
    DecodedSerialData, EventData, EventType, LocationData, MessageData, MotionError, MotionMessage,
    RemoteData, SerialData, motion,
};
use omnia_sdk::api::{Client, messaging};
pub use passenger_count::{PassengerCountMessage, Trip, Vehicle, passenger_count};
#[cfg(feature = "god-mode")]
pub use set_trip::{SET_TRIP, SetTripReply, SetTripRequest, set_trip};
pub use train_avl::{TrainAvlMessage, train_avl};
pub use trip::TIMEZONE;
pub use vehicle_info::{VEHICLE_INFO, VehicleInfoReply, VehicleInfoRequest, vehicle_info};

/// The stem's routes, merged onto the root router.
pub fn router<P: Capabilities>() -> axum::Router<Client<P>> {
    common::routers([
        // one entry per part with routes, each added at the end
        vehicle_info::routes(),
        #[cfg(feature = "god-mode")]
        set_trip::routes(),
    ])
}

/// The stem's topics, registered on the root messaging router under `env`.
#[must_use]
pub fn topics<P: Capabilities>(router: messaging::Router<P>, env: &str) -> messaging::Router<P> {
    common::topics(router, env).parts([
        // one entry per part with topics, each added at the end
        motion::topics,
        train_avl::topics,
        passenger_count::topics,
    ])
}
