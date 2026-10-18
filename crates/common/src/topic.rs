//! Environment-agnostic topic suffixes.
//!
//! A publishing crate and its consumer name one constant here, so producers
//! and consumers agree on the surface by construction rather than by review.
//! Prefix a suffix with `{env}-` through [`crate::config::topic`] or
//! [`crate::config::topic_for`] before publishing or subscribing.

/// Raw Pulse XML train updates (pulse-connector to pulse-adapter).
pub const PULSE: &str = "realtime-pulse.v1";

/// Motion location events (pulse-adapter to gtfs-adapter).
pub const PULSE_TO_MOTION: &str = "realtime-pulse-to-motion.v1";

/// Motion AVL restricted to Motion-tagged trains (consumed by
/// gtfs-adapter).
pub const TRAIN_AVL: &str = "realtime-train-avl.v1";

/// Passenger occupancy updates (consumed by gtfs-adapter).
pub const PASSENGER_COUNT: &str = "realtime-passenger-count.v1";

/// Tally APC output (its downstream consumer is out of scope for the
/// exemplar).
pub const TALLY_APC: &str = "realtime-tally-apc.v2";

/// GTFS-realtime vehicle positions emitted by gtfs-adapter.
pub const GTFS_VP: &str = "realtime-gtfs-vp.v1";

/// Dead-reckoning events emitted by gtfs-adapter.
pub const DEAD_RECKONING: &str = "realtime-dead-reckoning.v1";
