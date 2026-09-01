//! # SQL examples
//!
//! The rich `wasi-sql` showcase: an agency/feed schema exercising the full
//! guest ORM surface. Where `pattern_examples::place` uses the ORM for one
//! upsert-plus-select pattern, this crate restores the complete relational
//! example that omnia's "Example tidy" trimmed:
//!
//! - [`SelectBuilder`] with `order_by_desc`, `limit`, and `where` filters
//! - [`InsertBuilder::from_entity`] with server-assigned ids (a max-id
//!   probe, then max + 1)
//! - [`UpdateBuilder`] with conditional `.set()`s — only the provided
//!   fields are written — plus a fetch-after-update reply
//! - [`DeleteBuilder`] with 404 on zero rows affected
//! - `entity!` with multi-column JOIN aliasing: [`FeedWithAgency`] selects
//!   three columns from the joined `agency` table
//! - Existence checks answering 404, and referential checks rejecting a
//!   feed for a missing agency
//!
//! Schema DDL goes through [`TableStore::exec`](omnia_guest::TableStore)
//! rather than the wasm-only `Connection`/`Statement` bindings the pre-trim
//! example used, so the handlers run unchanged against native mock
//! providers — see [`schema`].
//!
//! [`SelectBuilder`]: omnia_guest::orm::SelectBuilder
//! [`InsertBuilder::from_entity`]: omnia_guest::orm::InsertBuilder::from_entity
//! [`UpdateBuilder`]: omnia_guest::orm::UpdateBuilder
//! [`DeleteBuilder`]: omnia_guest::orm::DeleteBuilder

pub mod agency;
pub mod feed;
pub mod paths;
pub mod schema;

pub use crate::agency::{
    AgenciesReply, Agency, AgencyReply, CreateAgencyRequest, GetAgencyRequest, ListAgenciesRequest,
    UpdateAgencyRequest,
};
pub use crate::feed::{
    CreateFeedRequest, DeleteFeedReply, DeleteFeedRequest, Feed, FeedReply, FeedWithAgency,
    FeedsReply, FeedsWithAgencyReply, ListAgencyFeedsRequest, ListAllFeedsRequest,
};

/// Named connection configured by the host.
pub const CONNECTION: &str = "db";

/// Deserialize an id that may arrive as a JSON number or a numeric string.
///
/// The JSON-body route codecs merge path parameters into the body as JSON
/// *strings*, so `PATCH /examples/agencies/{id}` delivers `id` as `"1"`
/// while a body field would be `1`. This deserializer accepts both, keeping
/// the input structs typed as `i64`.
pub(crate) fn numeric_id<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct IdVisitor;

    impl serde::de::Visitor<'_> for IdVisitor {
        type Value = i64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("an integer id, as a number or a numeric string")
        }

        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<i64, E> {
            Ok(v)
        }

        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<i64, E> {
            i64::try_from(v).map_err(E::custom)
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<i64, E> {
            v.parse().map_err(E::custom)
        }
    }

    deserializer.deserialize_any(IdVisitor)
}
