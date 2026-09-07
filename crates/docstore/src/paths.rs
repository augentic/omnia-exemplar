//! HTTP paths for the docstore-example handlers.
//!
//! Path constants for mounting the handlers under `/examples/*`,
//! deliberately outside the canonical transit tables in
//! `acme_common::routes` — these routes are pedagogical, not part of the
//! Acme service surface. Named `paths` (not `routes`) because this crate's
//! [`route`](crate::route) module is the GTFS route collection.

/// Stop collection: list (GET) and create (POST).
pub const STOPS: &str = "/examples/stops";

/// One stop: get (GET), upsert (PUT), and delete (DELETE).
pub const STOP: &str = "/examples/stops/{id}";

/// Route collection: list (GET) and create (POST).
pub const ROUTES: &str = "/examples/routes";

/// One route: get (GET).
pub const ROUTE: &str = "/examples/routes/{id}";

/// Stop-time collection: list (GET) and create (POST).
pub const STOP_TIMES: &str = "/examples/stop-times";

/// One stop time: get (GET).
pub const STOP_TIME: &str = "/examples/stop-times/{id}";
