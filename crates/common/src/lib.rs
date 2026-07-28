//! # Acme Common
//!
//! Logic common to the (fictional) Acme transit domain: upstream API clients,
//! the configuration-key catalog, and the canonical route/topic tables.

pub mod block_mgt;
pub mod config;
pub mod fleet;
pub mod routes;

/// The local timezone of the (fictional) Acme transit network.
///
/// All schedule arithmetic (service dates, seconds-since-midnight offsets)
/// is performed in this zone. Acme deliberately runs on UTC to keep the
/// exemplar's fixtures reproducible — a real operator would set its actual
/// IANA zone here (e.g. `Pacific/Auckland`) and must not copy UTC-as-local.
pub const TIMEZONE: chrono_tz::Tz = chrono_tz::UTC;
