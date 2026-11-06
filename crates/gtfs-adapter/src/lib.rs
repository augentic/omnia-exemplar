//! # Motion GTFS adapter
//!
//! Transforms Motion AVL telemetry into GTFS-realtime vehicle positions.

pub mod block_mgt;
pub mod fleet;
#[cfg(feature = "god-mode")]
pub mod god_mode;
mod location;
pub mod motion;
pub mod passenger_count;
mod serial_data;
#[cfg(feature = "god-mode")]
pub mod set_trip;
mod state_keys;
pub mod train_avl;
pub mod trip;
pub mod vehicle_info;

use common::Capabilities;
use omnia_sdk::api::Handlers;

/// The stem's handlers, merged onto the root routers.
pub fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::from([
        // one entry per part, each added at the end
        motion::handlers(),
        passenger_count::handlers(),
        #[cfg(feature = "god-mode")]
        set_trip::handlers(),
        train_avl::handlers(),
        vehicle_info::handlers(),
    ])
}
