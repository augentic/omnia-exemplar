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
use omnia_sdk::api::{Client, messaging};

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
    common::topics(
        router,
        env,
        [
            // one entry per part with topics, each added at the end
            motion::topics,
            train_avl::topics,
            passenger_count::topics,
        ],
    )
}
