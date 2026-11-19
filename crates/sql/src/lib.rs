//! # SQL examples
//!
//! The rich `omnia:sql` showcase: an agency/feed schema exercising
//! parameterized SQL on the `default` [`Database`], rows decoded through
//! [`FromRow`] and [`Database::query_as`]:
//!
//! - `SELECT` with `ORDER BY ... DESC`, optional `LIMIT`, and `WHERE`
//!   filters
//! - `INSERT` with server-assigned ids (a max-id probe, then max + 1)
//! - Dynamic `UPDATE ... SET` writing only the provided fields, plus a
//!   fetch-after-update reply
//! - `DELETE` answering 404 on zero rows affected
//! - Multi-column `LEFT JOIN` aliasing: [`feed::FeedWithAgency`] selects
//!   three columns from the joined `agency` table
//! - Existence checks answering 404, and referential checks rejecting a
//!   feed for a missing agency
//!
//! [`Database`]: omnia_sdk::sql::database::Database
//! [`Database::query_as`]: omnia_sdk::sql::database::Database::query_as
//! [`FromRow`]: omnia_sdk::sql::FromRow

pub mod agency;
pub mod feed;
mod schema;

use omnia_sdk::api::Handlers;

/// The stem's handlers, merged onto the root router.
pub fn handlers() -> Handlers {
    Handlers::from([agency::handlers(), feed::handlers()])
}
