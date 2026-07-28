//! HTTP routes for the pattern-example operations.
//!
//! Served by the guest to instantiate the composed default WASM capability
//! implementations, deliberately outside the canonical transit tables in
//! `acme_common::routes` — these routes are pedagogical, not part of the
//! Acme service surface.

/// Decode-through-cache segment lookup.
pub const DECODE: &str = "/examples/patterns/decode";

/// Relational place upsert.
pub const PLACES: &str = "/examples/patterns/places";

/// Bounding-box + haversine nearby query.
pub const NEARBY: &str = "/examples/patterns/nearby";
