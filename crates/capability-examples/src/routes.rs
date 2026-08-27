//! HTTP routes for the capability-example handlers.
//!
//! Route constants for mounting the handlers under `/examples/*`,
//! deliberately outside the canonical transit tables in
//! `acme_common::routes`. The workspace-root guest does not wire them by
//! default.

/// Blobstore archive ingress.
pub const ARCHIVE: &str = "/examples/archive";

/// WebSocket broadcast alert.
pub const ALERT: &str = "/examples/alert";

/// Document-store note upsert.
pub const NOTE: &str = "/examples/note";

/// Table-store sensor reading.
pub const READING: &str = "/examples/reading";
