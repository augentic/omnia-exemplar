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

pub use handlers::motion::{
    DecodedSerialData, EventData, EventType, LocationData, MessageData, MotionMessage, RemoteData,
    SerialData, motion,
};
pub use handlers::passenger_count::{PassengerCountMessage, Trip, Vehicle, passenger_count};
#[cfg(feature = "god-mode")]
pub use handlers::set_trip::{SetTripReply, SetTripRequest, set_trip};
pub use handlers::train_avl::{TrainAvlMessage, train_avl};
pub use handlers::vehicle_info::{VehicleInfoReply, VehicleInfoRequest, vehicle_info};
use omnia_guest::Error;
use thiserror::Error;

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
