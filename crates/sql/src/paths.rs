//! HTTP paths for the SQL-example handlers.
//!
//! Path constants for mounting the handlers under `/examples/*`,
//! deliberately apart from the transit crates' paths — these routes are
//! pedagogical, not part of the Acme service surface.

/// Agency collection: list (GET) and create (POST).
pub const AGENCIES: &str = "/examples/agencies";

/// One agency: get (GET) and partial update (PATCH).
pub const AGENCY: &str = "/examples/agencies/{id}";

/// Feeds of one agency: list (GET) and create (POST).
pub const AGENCY_FEEDS: &str = "/examples/agencies/{agency_id}/feeds";

/// All feeds joined with their agency details (GET).
pub const FEEDS: &str = "/examples/feeds";

/// One feed: delete (DELETE).
pub const FEED: &str = "/examples/feeds/{id}";
