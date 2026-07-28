//! Canonical HTTP routes and messaging topics.
//!
//! Both guests (`guests/typed` and `guests/axum`) build their routers from
//! these tables, so the two guest styles serve the same surface by
//! construction rather than by review. Domain crates publish to the same
//! topic constants their consumers subscribe to.

/// HTTP ingress paths served by both guests.
pub mod http {
    /// Tally passenger-count (APC) ingress.
    pub const APC: &str = "/api/apc";

    /// Pulse SOAP/XML position ingress.
    pub const PULSE_XML: &str = "/inbound/xml";

    /// Vehicle info lookup.
    pub const VEHICLE_INFO: &str = "/info/{vehicle_id}";

    /// God-mode trip override (registered only when the `god-mode` feature
    /// is enabled).
    pub const SET_TRIP: &str = "/god-mode/set-trip/{vehicle_id}/{trip_id}";
}

/// Environment-agnostic topic suffixes. Prefix with `{env}-` via
/// [`crate::config::topic`] before publishing or subscribing.
pub mod topic {
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
}
