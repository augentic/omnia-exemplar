//! Schema DDL, run on the `default` database before every handler's own
//! statements.

use omnia_sdk::Result;
use omnia_sdk::sql::database::Database;
use omnia_sdk::sql::{DEFAULT, store};

pub mod sql {
    pub const CREATE_AGENCY: &str = "CREATE TABLE IF NOT EXISTS agency (
        agency_id BIGINT PRIMARY KEY,
        name TEXT NOT NULL,
        url TEXT,
        timezone TEXT,
        created_at TEXT NOT NULL
    )";

    pub const CREATE_FEED: &str = "CREATE TABLE IF NOT EXISTS feed (
        feed_id BIGINT PRIMARY KEY,
        agency_id BIGINT NOT NULL,
        description TEXT NOT NULL,
        created_at TEXT NOT NULL
    )";
}

// Called at the top of every handler: omnia creates one guest instance per
// request, so there is no process-lifetime initialisation hook. The
// `IF NOT EXISTS` guard keeps the repeat cheap.
pub async fn open() -> Result<Database> {
    let db = store::open(DEFAULT.to_owned()).await?;
    db.exec(sql::CREATE_AGENCY, &[]).await?;
    db.exec(sql::CREATE_FEED, &[]).await?;
    Ok(db)
}
