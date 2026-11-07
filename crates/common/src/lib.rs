//! # Common
//!
//! What every stem shares: the capability bound a stem's handlers take
//! and the topic table a publisher and its consumer agree on.
//!
//! A stem's `handlers()` is stateless and keyed by path or topic, a stem
//! exposes it whatever parts it has, and the root merges every stem's
//! handlers onto its router over one [`Client`](omnia_sdk::api::Client).

pub mod topic;

use omnia_sdk::{
    BlobStore, Broadcast, Config, DocumentStore, HttpRequest, Identity, Publish, StateStore,
    TableStore, Vault,
};

/// Every capability a stem may bound its provider on.
///
/// A stem's `handlers` takes any provider under this bound, so a stem
/// reaching a capability no stem reached before changes no signature.
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
    + Vault
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
        + Vault
        + Send
        + Sync
        + 'static
{
}
