//! # SQL examples
//!
//! The rich `omnia:sql` showcase: an agency/feed schema exercising
//! parameterized SQL over [`SqlDb`] on the provider's `default` database,
//! rows decoded through [`FromRow`] and [`SqlDbExt::query_as`]:
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
//! Schema DDL goes through [`SqlDb::execute`] rather than wasm-only
//! bindings so the handlers run unchanged against native mock providers.
//!
//! [`FromRow`]: omnia_sdk::sql::FromRow
//! [`SqlDbExt::query_as`]: omnia_sdk::SqlDbExt::query_as

pub mod agency;
pub mod feed;
mod schema;

use omnia_sdk::SqlDb;
use omnia_sdk::api::Handlers;

/// The capabilities the stem's handlers reach, implemented by every provider that has them.
pub trait Requires: SqlDb + Send + Sync + 'static {}

impl<P> Requires for P where P: SqlDb + Send + Sync + 'static {}

/// The stem's handlers, merged onto the root routers.
pub fn handlers<P: Requires>() -> Handlers<P> {
    Handlers::from([agency::handlers(), feed::handlers()])
}
