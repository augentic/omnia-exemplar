//! Motion GTFS adapter operations.

pub mod motion;
pub mod passenger_count;
pub mod set_trip;
pub mod train_avl;
pub mod vehicle_info;

pub use motion::*;
pub use passenger_count::*;
pub use set_trip::*;
pub use train_avl::*;
pub use vehicle_info::*;
