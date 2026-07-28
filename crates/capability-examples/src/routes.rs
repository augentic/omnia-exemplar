//! HTTP routes for the capability-example operations.
//!
//! Served by `guests/typed` only: these routes exist to instantiate the
//! default WASM capability implementations in a real guest, deliberately
//! outside the canonical transit tables in `acme_common::routes` that both
//! guest styles share.

/// Blobstore archive ingress.
pub const ARCHIVE: &str = "/examples/archive";

/// WebSocket broadcast alert.
pub const ALERT: &str = "/examples/alert";

/// Document-store note upsert.
pub const NOTE: &str = "/examples/note";

/// Table-store sensor reading.
pub const READING: &str = "/examples/reading";
