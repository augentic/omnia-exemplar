//! # Acme Common
//!
//! Logic common to the (fictional) Acme transit domain: upstream API clients,
//! the configuration-key catalog, the topic table a publisher and its consumer
//! agree on, and the capability bound every crate's routers take.

pub mod block_mgt;
pub mod config;
pub mod fleet;
pub mod topic;

use omnia_sdk::{
    BlobStore, Broadcast, Config, DocumentStore, HttpRequest, Identity, Publish, StateStore,
    TableStore,
};

/// The local timezone of the (fictional) Acme transit network.
///
/// All schedule arithmetic (service dates, seconds-since-midnight offsets)
/// is performed in this zone. Acme deliberately runs on UTC to keep the
/// exemplar's fixtures reproducible — a real operator would set its actual
/// IANA zone here (e.g. `Pacific/Auckland`) and must not copy UTC-as-local.
pub const TIMEZONE: chrono_tz::Tz = chrono_tz::UTC;

/// Every capability a crate may bound its provider on.
///
/// A crate's `router` and `topics` take any provider under this bound, so a
/// handler reaching a capability no handler reached before changes no
/// signature.
pub trait Capabilities:
    BlobStore
    + Broadcast
    + Config
    + DocumentStore
    + HttpRequest
    + Identity
    + Publish
    + StateStore
    + TableStore
    + Send
    + Sync
    + 'static
{
}

impl<P> Capabilities for P where
    P: BlobStore
        + Broadcast
        + Config
        + DocumentStore
        + HttpRequest
        + Identity
        + Publish
        + StateStore
        + TableStore
        + Send
        + Sync
        + 'static
{
}
