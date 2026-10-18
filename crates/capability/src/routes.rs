//! HTTP routes for the capability-example handlers.
//!
//! Route constants for mounting the handlers under `/examples/*`,
//! deliberately apart from the transit crates' paths. The crate's
//! [`router`](crate::router) serves all four as body-JSON POSTs.

/// Blobstore archive ingress.
pub const ARCHIVE: &str = "/examples/archive";

/// WebSocket broadcast alert.
pub const ALERT: &str = "/examples/alert";

/// Document-store note upsert.
pub const NOTE: &str = "/examples/note";

/// Table-store sensor reading.
pub const READING: &str = "/examples/reading";
