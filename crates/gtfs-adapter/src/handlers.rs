//! Motion GTFS adapter handlers.
//!
//! Each handler and the payload types callers need are re-exported from
//! the crate root; these modules stay crate-internal.

pub mod motion;
pub mod passenger_count;
#[cfg(feature = "god-mode")]
pub mod set_trip;
pub mod train_avl;
pub mod vehicle_info;
