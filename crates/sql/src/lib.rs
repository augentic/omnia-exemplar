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
//! - Multi-column `LEFT JOIN` aliasing: [`FeedWithAgency`] selects three
//!   columns from the joined `agency` table
//! - Existence checks answering 404, and referential checks rejecting a
//!   feed for a missing agency
//!
//! Schema DDL goes through [`TableStore::exec`] rather than wasm-only
//! bindings so the handlers run unchanged against native mock providers —
//! see [`schema`].
//!
//! [`TableStore`]: omnia_sdk::TableStore
//! [`TableStore::exec`]: omnia_sdk::TableStore::exec

pub mod agency;
pub mod feed;
pub mod schema;

use common::Capabilities;
use omnia_sdk::api::Client;

pub use crate::agency::{
    AGENCIES, AGENCY, AgenciesReply, Agency, AgencyReply, CreateAgencyRequest, GetAgencyRequest,
    ListAgenciesRequest, UpdateAgencyRequest, create_agency, get_agency, list_agencies,
    update_agency,
};
pub use crate::feed::{
    AGENCY_FEEDS, CreateFeedRequest, DeleteFeedReply, DeleteFeedRequest, FEED, FEEDS, Feed,
    FeedReply, FeedWithAgency, FeedsReply, FeedsWithAgencyReply, ListAgencyFeedsRequest,
    ListAllFeedsRequest, create_feed, delete_feed, list_agency_feeds, list_all_feeds,
};
pub use crate::schema::CONNECTION;

/// The stem's routes, merged onto the root router.
pub fn router<P: Capabilities>() -> axum::Router<Client<P>> {
    common::routers([
        // one entry per part with routes, each added at the end
        agency::routes(),
        feed::routes(),
    ])
}
