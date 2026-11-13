//! Schema DDL through the [`SqlDb`] capability.
//!
//! The pre-trim omnia example created its tables through the wasm-only
//! `Connection`/`Statement`/`readwrite` bindings. Routing the DDL through
//! [`SqlDb::execute`] instead keeps the handlers runnable against native
//! `omnia-test` doubles — a deliberate, documented deviation.

use omnia_sdk::SqlDb;
use omnia_sdk::capability::Result;

pub mod sql {
    pub const CREATE_AGENCY: &str = "CREATE TABLE IF NOT EXISTS agency (
        agency_id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        url TEXT,
        timezone TEXT,
        created_at TEXT NOT NULL
    )";

    pub const CREATE_FEED: &str = "CREATE TABLE IF NOT EXISTS feed (
        feed_id INTEGER PRIMARY KEY,
        agency_id INTEGER NOT NULL,
        description TEXT NOT NULL,
        created_at TEXT NOT NULL
    )";
}

// Called at the top of every handler: omnia creates one guest instance per
// request, so there is no process-lifetime initialisation hook. The
// `IF NOT EXISTS` guard keeps the repeat cheap.
pub async fn ensure<P: SqlDb>(db: &P) -> Result<()> {
    db.execute(sql::CREATE_AGENCY, &[]).await?;
    db.execute(sql::CREATE_FEED, &[]).await?;
    Ok(())
}
