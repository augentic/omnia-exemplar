//! # Common
//!
//! What every stem shares: the capability bound a stem's routers take, the
//! two folds a stem's entry composes its parts with, the topic table a
//! publisher and its consumer agree on, the configuration helpers that
//! qualify a topic for the deployment's environment, and typed SQL column
//! reads.

pub mod config;
pub mod topic;

use omnia_sdk::api::{Client, messaging};
use omnia_sdk::axum::Router;
use omnia_sdk::sql::{DataType, Field, Row};
use omnia_sdk::{
    BlobStore, Broadcast, Config, DocumentStore, HttpRequest, Identity, Publish, Result,
    StateStore, TableStore, Vault, server_error,
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

/// Typed column reads over an [`omnia_sdk::sql::Row`], so a handler never matches on [`DataType`] by hand.
pub trait RowExt {
    /// Read a required 64-bit integer column.
    ///
    /// # Errors
    ///
    /// Returns an [`omnia_sdk::Error`] when `column` is missing, null, or not an integer.
    fn get_i64(&self, column: &str) -> Result<i64>;

    /// Read a required 64-bit floating-point column.
    ///
    /// # Errors
    ///
    /// Returns an [`omnia_sdk::Error`] when `column` is missing, null, or not a float.
    fn get_f64(&self, column: &str) -> Result<f64>;

    /// Read a required string column.
    ///
    /// # Errors
    ///
    /// Returns an [`omnia_sdk::Error`] when `column` is missing, null, or not a string.
    fn get_str(&self, column: &str) -> Result<&str>;

    /// Read an optional string column.
    ///
    /// # Errors
    ///
    /// Returns an [`omnia_sdk::Error`] when `column` is missing or not a string.
    fn get_opt_str(&self, column: &str) -> Result<Option<&str>>;
}

impl RowExt for Row {
    fn get_i64(&self, column: &str) -> Result<i64> {
        match field(self, column)? {
            DataType::Int64(Some(value)) => Ok(*value),
            DataType::Int32(Some(value)) => Ok(i64::from(*value)),
            DataType::Int64(None) | DataType::Int32(None) => {
                Err(server_error!("column `{column}` is null"))
            }
            other => Err(server_error!("column `{column}` is not an integer: {other:?}")),
        }
    }

    fn get_f64(&self, column: &str) -> Result<f64> {
        match field(self, column)? {
            DataType::Double(Some(value)) => Ok(*value),
            DataType::Float(Some(value)) => Ok(f64::from(*value)),
            DataType::Double(None) | DataType::Float(None) => {
                Err(server_error!("column `{column}` is null"))
            }
            other => Err(server_error!("column `{column}` is not a float: {other:?}")),
        }
    }

    fn get_str(&self, column: &str) -> Result<&str> {
        match field(self, column)? {
            DataType::Str(Some(value)) => Ok(value.as_str()),
            DataType::Str(None) => Err(server_error!("column `{column}` is null")),
            other => Err(server_error!("column `{column}` is not a string: {other:?}")),
        }
    }

    fn get_opt_str(&self, column: &str) -> Result<Option<&str>> {
        match field(self, column)? {
            DataType::Str(value) => Ok(value.as_deref()),
            other => Err(server_error!("column `{column}` is not a string: {other:?}")),
        }
    }
}

fn field<'a>(row: &'a Row, column: &str) -> Result<&'a DataType> {
    row.fields
        .iter()
        .find(|Field { name, .. }| name == column)
        .map(|field| &field.value)
        .ok_or_else(|| server_error!("row lacks column `{column}`"))
}

/// An integer id that arrives as a JSON number or as a path parameter's string (`"42"`).
///
/// # Errors
///
/// Returns a deserialization error when the value is neither an integer nor a numeric string.
pub fn numeric_id<'de, D>(deserializer: D) -> std::result::Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct IdVisitor;

    impl serde::de::Visitor<'_> for IdVisitor {
        type Value = i64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("an integer id, as a number or a numeric string")
        }

        fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<i64, E> {
            Ok(v)
        }

        fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<i64, E> {
            i64::try_from(v).map_err(E::custom)
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<i64, E> {
            v.parse().map_err(E::custom)
        }
    }

    deserializer.deserialize_any(IdVisitor)
}
