//! Optimistic-concurrency update over [`StateStore::cas`].
//!
//! Demonstrates compare-and-swap state updates where a concurrent write is
//! rejected with `409 Conflict`. Because `omnia_sdk::Error` covers only
//! `400`, `404`, `500`, and `502`, the handler answers `Result<QuotaRecord,
//! QuotaError>` and maps [`CasError::Conflict`] to [`StatusCode::CONFLICT`]
//! in its [`From<QuotaError> for HttpError`] implementation.

use anyhow::Context as _;
use common::Capabilities;
use http::StatusCode;
use omnia_sdk::api::http::put;
use omnia_sdk::api::{Client, Context};
use omnia_sdk::{CasError, HttpError, StateStore, not_found, server_error};
use serde::{Deserialize, Serialize};

/// Optimistic-concurrency quota update route.
pub const QUOTA: &str = "/examples/patterns/quotas/{tenant}";

pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {
    axum::Router::new().route(QUOTA, put(update_quota))
}

/// State-store key for a tenant's quota record.
#[must_use]
pub fn quota_key(tenant: &str) -> String {
    format!("pattern:quota:{tenant}")
}

/// A versioned quota record stored in [`StateStore`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuotaRecord {
    /// Tenant identifier.
    pub tenant: String,
    /// Monotonically increasing revision number.
    pub version: u64,
    /// Allowed request limit.
    pub limit: u64,
}

/// Update a tenant's quota, guarded by its expected version.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpdateQuotaRequest {
    /// Tenant identifier (path parameter).
    pub tenant: String,
    /// Revision the caller observed (`0` when creating the initial record).
    pub expected_version: u64,
    /// Replacement request limit.
    pub limit: u64,
}

/// Why a quota update could not be applied.
#[derive(Debug, thiserror::Error)]
pub enum QuotaError {
    /// Another writer modified the record since `expected_version` was read.
    #[error("quota for `{0}` changed concurrently")]
    Conflict(String),
    /// The host or a precondition failed.
    #[error(transparent)]
    Host(#[from] omnia_sdk::Error),
}

impl From<QuotaError> for HttpError {
    fn from(error: QuotaError) -> Self {
        match error {
            QuotaError::Conflict(_) => Self::new(StatusCode::CONFLICT, error.to_string()),
            QuotaError::Host(inner) => inner.into(),
        }
    }
}

/// Atomically updates a tenant's quota when `expected_version` matches the
/// stored version, or creates version `1` when `expected_version` is `0`.
///
/// # Errors
///
/// Returns [`QuotaError::Conflict`] (`409 Conflict`) when the stored version
/// differs or a concurrent write wins the compare-and-swap, `not_found` when
/// `expected_version > 0` and no record exists, or [`QuotaError::Host`] when
/// storage or serialization fails.
pub async fn update_quota<P>(
    input: UpdateQuotaRequest, context: Context<P>,
) -> Result<QuotaRecord, QuotaError>
where
    P: StateStore,
{
    let provider = context.provider();
    let key = quota_key(&input.tenant);
    let current = StateStore::get(provider, &key).await.map_err(omnia_sdk::Error::from)?;

    let expected_bytes = match (&current, input.expected_version) {
        (None, 0) => None,
        (None, _) => {
            return Err(not_found!("quota for `{}` not found", input.tenant).into());
        }
        (Some(bytes), expected) => {
            let record: QuotaRecord = serde_json::from_slice(bytes)
                .context("parsing stored quota")
                .map_err(omnia_sdk::Error::from)?;
            if record.version != expected {
                return Err(QuotaError::Conflict(input.tenant));
            }
            Some(bytes.as_slice())
        }
    };

    let next = QuotaRecord {
        tenant: input.tenant.clone(),
        version: input.expected_version + 1,
        limit: input.limit,
    };
    let next_bytes =
        serde_json::to_vec(&next).context("serializing quota").map_err(omnia_sdk::Error::from)?;

    match StateStore::cas(provider, &key, expected_bytes, &next_bytes).await {
        Ok(()) => Ok(next),
        Err(CasError::Conflict(_)) => Err(QuotaError::Conflict(input.tenant)),
        Err(CasError::Store(err)) => Err(server_error!("{err}").into()),
    }
}
