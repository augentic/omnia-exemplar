//! # SQL examples
//!
//! The rich `wasi-sql` showcase: an agency/feed schema exercising
//! parameterized SQL over [`TableStore`], rows decoded through
//! [`FromRow`] and [`TableStore::query_as`]:
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
//! Schema DDL goes through [`TableStore::exec`] rather than wasm-only
//! bindings so the handlers run unchanged against native mock providers.
//!
//! [`FromRow`]: omnia_sdk::sql::FromRow
//! [`TableStore`]: omnia_sdk::TableStore
//! [`TableStore::exec`]: omnia_sdk::TableStore::exec
//! [`TableStore::query_as`]: omnia_sdk::TableStore::query_as

pub mod agency;
pub mod feed;
mod schema;

use common::Capabilities;
use omnia_sdk::api::Handlers;

/// The stem's handlers, merged onto the root routers.
pub fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::from([agency::handlers(), feed::handlers()])
}
