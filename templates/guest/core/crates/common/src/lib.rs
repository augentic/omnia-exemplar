//! # Common
//!
//! What every stem shares: the capability bound a stem's routers take, the
//! two folds a stem's entry composes its parts with, the topic table a
//! publisher and its consumer agree on, and the configuration helpers that
//! qualify a topic for the deployment's environment.

pub mod config;
pub mod topic;

use omnia_sdk::api::{Client, messaging};
use omnia_sdk::axum::Router;
use omnia_sdk::{
    BlobStore, Broadcast, Config, DocumentStore, HttpRequest, Identity, Publish, StateStore,
    TableStore,
};

/// Every capability a stem may bound its provider on.
///
/// A stem's `router` and `topics` take any provider under this bound, so a
/// stem reaching a capability no stem reached before changes no signature.
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

/// Merge the routers of a stem's parts onto one router.
///
/// A stem's `router` is this fold over its parts' `routes()`, one entry per
/// part, so a stem with no route yet is the fold over none. Merging panics
/// where two parts serve one path under one method.
pub fn routers<P, const N: usize>(parts: [Router<Client<P>>; N]) -> Router<Client<P>>
where
    P: Send + Sync + 'static,
{
    parts.into_iter().fold(Router::new(), Router::merge)
}

/// A part's topics: a registration on the messaging router under an environment.
pub type Topics<P> = fn(messaging::Router<P>, &str) -> messaging::Router<P>;

/// The messaging router and the environment a stem registers its parts' topics under.
///
/// Answered by [`topics`]; [`parts`](Self::parts) finishes the registration.
#[must_use]
pub struct Registering<'env, P: Send + Sync + 'static> {
    router: messaging::Router<P>,
    env: &'env str,
}

/// Begin registering a stem's topics on `router` under `env`.
///
/// A stem's `topics` is `common::topics(router, env).parts([..])` over its
/// parts' `topics`, one entry per part, so a stem with no topic yet
/// registers none and answers the router as it came.
pub const fn topics<P: Send + Sync + 'static>(
    router: messaging::Router<P>, env: &str,
) -> Registering<'_, P> {
    Registering { router, env }
}

impl<P: Send + Sync + 'static> Registering<'_, P> {
    /// Register each part's topics, in order, and answer the router.
    ///
    /// Registering panics where two parts consume one topic.
    #[must_use]
    pub fn parts<const N: usize>(self, parts: [Topics<P>; N]) -> messaging::Router<P> {
        parts.into_iter().fold(self.router, |router, part| part(router, self.env))
    }
}
