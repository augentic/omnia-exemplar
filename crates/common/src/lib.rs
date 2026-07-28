//! # Transit Common
//!
//! Logic common to the transit domain.

pub mod block_mgt;
pub mod fleet;

/// The local timezone of the (fictional) Acme transit network.
///
/// All schedule arithmetic (service dates, seconds-since-midnight offsets)
/// is performed in this zone.
pub const TIMEZONE: chrono_tz::Tz = chrono_tz::UTC;
