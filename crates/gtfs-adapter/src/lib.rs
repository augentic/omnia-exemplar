//! # Motion GTFS adapter
//!
//! Transforms Motion AVL telemetry into GTFS-realtime vehicle positions.

#[cfg(feature = "god-mode")]
mod god_mode;
mod handlers;
mod location;
mod serial_data;
mod state_keys;
mod trip;

use acme_common::{Capabilities, config, topic};
pub use handlers::motion::{
    DecodedSerialData, EventData, EventType, LocationData, MessageData, MotionMessage, RemoteData,
    SerialData, motion,
};
pub use handlers::passenger_count::{PassengerCountMessage, Trip, Vehicle, passenger_count};
#[cfg(feature = "god-mode")]
pub use handlers::set_trip::{SetTripReply, SetTripRequest, set_trip};
pub use handlers::train_avl::{TrainAvlMessage, train_avl};
pub use handlers::vehicle_info::{VehicleInfoReply, VehicleInfoRequest, vehicle_info};
use omnia_sdk::Error;
use omnia_sdk::api::Client;
use omnia_sdk::api::http::get;
#[cfg(feature = "god-mode")]
use omnia_sdk::api::http::post;
use omnia_sdk::api::messaging::{self, consume};
use thiserror::Error;

/// Vehicle info lookup.
pub const VEHICLE_INFO: &str = "/info/{vehicle_id}";

/// God-mode trip override (served only when the `god-mode` feature is
/// enabled).
#[cfg(feature = "god-mode")]
pub const SET_TRIP: &str = "/god-mode/set-trip/{vehicle_id}/{trip_id}";

/// The crate's routes, merged onto the guest's router.
pub fn router<P: Capabilities>() -> axum::Router<Client<P>> {
    let router = axum::Router::new().route(VEHICLE_INFO, get(vehicle_info));
    #[cfg(feature = "god-mode")]
    let router = router.route(SET_TRIP, post(set_trip));
    router
}

/// The crate's topics, registered on the guest's messaging router under `env`.
#[must_use]
pub fn topics<P: Capabilities>(router: messaging::Router<P>, env: &str) -> messaging::Router<P> {
    router
        .route(config::topic_for(env, topic::PULSE_TO_MOTION), consume(motion))
        .route(config::topic_for(env, topic::TRAIN_AVL), consume(train_avl))
        .route(config::topic_for(env, topic::PASSENGER_COUNT), consume(passenger_count))
}

/// Errors raised while validating an inbound Motion message.
#[derive(Error, Debug)]
enum MotionError {
    /// The message timestamp is invalid (too old or future-dated).
    #[error("{0}")]
    BadTime(String),
}

impl MotionError {
    fn code(&self) -> String {
        match self {
            Self::BadTime(_) => "bad_time".to_string(),
        }
    }
}

impl From<MotionError> for Error {
    fn from(err: MotionError) -> Self {
        Self::BadRequest {
            code: err.code(),
            description: err.to_string(),
        }
    }
}
