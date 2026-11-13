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

use omnia_sdk::api::Handlers;
use omnia_sdk::{Config, HttpClient, Identity, Publisher, StateStore};

/// The capabilities the stem's handlers reach, implemented by every provider that has them.
pub trait Requires:
    Config + HttpClient + Identity + Publisher + StateStore + Send + Sync + 'static
{
}

impl<P> Requires for P where
    P: Config + HttpClient + Identity + Publisher + StateStore + Send + Sync + 'static
{
}

/// The stem's handlers, merged onto the root routers.
pub fn handlers<P: Requires>() -> Handlers<P> {
    Handlers::from([
        motion::handlers(),
        passenger_count::handlers(),
        #[cfg(feature = "god-mode")]
        set_trip::handlers(),
        train_avl::handlers(),
        vehicle_info::handlers(),
    ])
}
