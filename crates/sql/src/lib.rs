//! # SQL examples
//!
//! The rich `wasi-sql` showcase: an agency/feed schema exercising
//! parameterized SQL over [`TableStore`] and [`common::RowExt`]:
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
//! [`TableStore`]: omnia_sdk::TableStore
//! [`TableStore::exec`]: omnia_sdk::TableStore::exec

pub mod agency;
pub mod feed;
mod schema;

use common::Capabilities;
use omnia_sdk::api::{http, messaging};

/// The stem's routes, merged onto the root router.
pub fn routes<P: Capabilities>() -> http::Routes<P> {
    http::Routes::from([
        // one entry per part with routes, each added at the end
        agency::routes(),
        feed::routes(),
    ])
}

/// The stem's topics, merged onto the root messaging router.
pub fn topics<P: Capabilities>() -> messaging::Routes<P> {
    messaging::Routes::from([
        // one entry per part with topics, each added at the end
    ])
}
