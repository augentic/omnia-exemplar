//! Schema DDL through the [`TableStore`] capability.
//!
//! The pre-trim omnia example created its tables through the wasm-only
//! `Connection`/`Statement`/`readwrite` bindings. Routing the DDL through
//! [`TableStore::exec`] instead keeps the handlers runnable against native
//! `omnia-test` doubles — a deliberate, documented deviation.

use omnia_guest::TableStore;

use crate::CONNECTION;

/// SQL executed to create the schema.
pub mod sql {
    /// Agency table.
    pub const CREATE_AGENCY: &str = "CREATE TABLE IF NOT EXISTS agency (
        agency_id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        url TEXT,
        timezone TEXT,
        created_at TEXT NOT NULL
    )";

    /// Feed table (references agency via `agency_id`).
    pub const CREATE_FEED: &str = "CREATE TABLE IF NOT EXISTS feed (
        feed_id INTEGER PRIMARY KEY,
        agency_id INTEGER NOT NULL,
        description TEXT NOT NULL,
        created_at TEXT NOT NULL
    )";
}

/// Create the schema when it is missing.
///
/// Called at the top of every handler: omnia creates one guest instance per
/// request, so there is no process-lifetime initialisation hook. The
/// `IF NOT EXISTS` guard keeps the repeat cheap.
///
/// # Errors
///
/// Returns an error when a DDL statement fails to execute.
pub async fn ensure<P: TableStore>(provider: &P) -> anyhow::Result<()> {
    TableStore::exec(provider, CONNECTION.to_string(), sql::CREATE_AGENCY.to_string(), vec![])
        .await?;
    TableStore::exec(provider, CONNECTION.to_string(), sql::CREATE_FEED.to_string(), vec![])
        .await?;
    Ok(())
}
