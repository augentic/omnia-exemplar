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

/// Register the topics of a stem's parts on `router` under `env`, in order.
///
/// A stem's `topics` is `common::topics(router, env, [..])` over its parts'
/// `topics`, one entry per part, so a stem with no topic yet registers none
/// and answers the router as it came. Registering panics where two parts
/// consume one topic.
#[must_use]
pub fn topics<P, const N: usize>(
    router: messaging::Router<P>, env: &str, parts: [Topics<P>; N],
) -> messaging::Router<P>
where
    P: Send + Sync + 'static,
{
    parts.into_iter().fold(router, |router, part| part(router, env))
}

/// Typed column reads over an [`omnia_sdk::sql::Row`], so a handler never matches on [`DataType`] by hand.
pub trait RowExt {
    /// Read a required 64-bit integer column.
    ///
    /// # Errors
    ///
    /// Returns an [`omnia_sdk::Error`] when `column` is missing, null, or not an integer.
    fn get_i64(&self, column: &str) -> Result<i64> {
        self.get_opt_i64(column)?.ok_or_else(|| server_error!("column `{column}` is null"))
    }

    /// Read an optional 64-bit integer column.
    ///
    /// # Errors
    ///
    /// Returns an [`omnia_sdk::Error`] when `column` is missing or not an integer.
    fn get_opt_i64(&self, column: &str) -> Result<Option<i64>>;

    /// Read a required 64-bit floating-point column.
    ///
    /// # Errors
    ///
    /// Returns an [`omnia_sdk::Error`] when `column` is missing, null, or not a float.
    fn get_f64(&self, column: &str) -> Result<f64> {
        self.get_opt_f64(column)?.ok_or_else(|| server_error!("column `{column}` is null"))
    }

    /// Read an optional 64-bit floating-point column.
    ///
    /// # Errors
    ///
    /// Returns an [`omnia_sdk::Error`] when `column` is missing or not a float.
    fn get_opt_f64(&self, column: &str) -> Result<Option<f64>>;

    /// Read a required boolean column.
    ///
    /// # Errors
    ///
    /// Returns an [`omnia_sdk::Error`] when `column` is missing, null, or not a boolean.
    fn get_bool(&self, column: &str) -> Result<bool> {
        self.get_opt_bool(column)?.ok_or_else(|| server_error!("column `{column}` is null"))
    }

    /// Read an optional boolean column.
    ///
    /// # Errors
    ///
    /// Returns an [`omnia_sdk::Error`] when `column` is missing or not a boolean.
    fn get_opt_bool(&self, column: &str) -> Result<Option<bool>>;

    /// Read a required string column.
    ///
    /// # Errors
    ///
    /// Returns an [`omnia_sdk::Error`] when `column` is missing, null, or not a string.
    fn get_str(&self, column: &str) -> Result<&str> {
        self.get_opt_str(column)?.ok_or_else(|| server_error!("column `{column}` is null"))
    }

    /// Read an optional string column.
    ///
    /// # Errors
    ///
    /// Returns an [`omnia_sdk::Error`] when `column` is missing or not a string.
    fn get_opt_str(&self, column: &str) -> Result<Option<&str>>;
}

impl RowExt for Row {
    fn get_opt_i64(&self, column: &str) -> Result<Option<i64>> {
        match field(self, column)? {
            DataType::Int64(value) => Ok(*value),
            DataType::Int32(value) => Ok((*value).map(i64::from)),
            DataType::Uint32(value) => Ok((*value).map(i64::from)),
            DataType::Uint64(value) => (*value)
                .map(i64::try_from)
                .transpose()
                .map_err(|err| server_error!("column `{column}` overflows i64: {err}")),
            other => Err(server_error!("column `{column}` is not an integer: {other:?}")),
        }
    }

    fn get_opt_f64(&self, column: &str) -> Result<Option<f64>> {
        match field(self, column)? {
            DataType::Double(value) => Ok(*value),
            DataType::Float(value) => Ok((*value).map(f64::from)),
            other => Err(server_error!("column `{column}` is not a float: {other:?}")),
        }
    }

    fn get_opt_bool(&self, column: &str) -> Result<Option<bool>> {
        match field(self, column)? {
            DataType::Boolean(value) => Ok(*value),
            other => Err(server_error!("column `{column}` is not a boolean: {other:?}")),
        }
    }

    fn get_opt_str(&self, column: &str) -> Result<Option<&str>> {
        match field(self, column)? {
            DataType::Str(value)
            | DataType::Date(value)
            | DataType::Time(value)
            | DataType::Timestamp(value) => Ok(value.as_deref()),
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
