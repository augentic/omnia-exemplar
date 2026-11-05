//! Optimistic-concurrency update over [`StateStore::cas`].
//!
//! Demonstrates compare-and-swap state updates where a concurrent write is
//! rejected with `409 Conflict`. Because `omnia_sdk::Error` covers only
//! `400`, `404`, `500`, and `502`, the handler answers `Result<QuotaRecord,
//! QuotaError>` and maps [`CasError::Conflict`] to [`StatusCode::CONFLICT`]
//! in its [`From<QuotaError> for HttpError`] implementation.

use common::Capabilities;
use http::{HeaderValue, StatusCode};
use omnia_sdk::api::http::put;
use omnia_sdk::api::{Client, Context};
use omnia_sdk::{CasError, HttpError, StateStore, not_found, server_error};
use serde::{Deserialize, Serialize};

pub(crate) const QUOTA: &str = "/examples/patterns/quotas/{tenant}";

pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {
    axum::Router::new().route(QUOTA, put(update_quota))
}

pub(crate) fn quota_key(tenant: &str) -> String {
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
    #[error("quota for `{tenant}` changed concurrently (current version: {current_version:?})")]
    Conflict {
        /// Tenant whose quota conflicted.
        tenant: String,
        /// Version currently stored, when the store returned one.
        current_version: Option<u64>,
    },
    /// The host or a precondition failed.
    #[error(transparent)]
    Host(#[from] omnia_sdk::Error),
}

impl From<anyhow::Error> for QuotaError {
    fn from(error: anyhow::Error) -> Self {
        Self::Host(error.into())
    }
}

impl From<QuotaError> for HttpError {
    fn from(error: QuotaError) -> Self {
        match error {
            QuotaError::Conflict {
                ref tenant,
                current_version,
            } => {
                let body = serde_json::json!({
                    "error": "conflict",
                    "message": error.to_string(),
                    "tenant": tenant,
                    "current_version": current_version,
                });
                Self::with_body(
                    StatusCode::CONFLICT,
                    HeaderValue::from_static("application/json"),
                    body.to_string().into_bytes(),
                )
            }
            QuotaError::Host(inner) => inner.into(),
        }
    }
}

pub(crate) async fn update_quota<P>(
    input: UpdateQuotaRequest, context: Context<P>,
) -> Result<QuotaRecord, QuotaError>
where
    P: StateStore,
{
    let provider = context.provider();
    let key = quota_key(&input.tenant);
    let current = StateStore::get(provider, &key).await?;

    let expected_bytes = match (&current, input.expected_version) {
        (None, 0) => None,
        (None, _) => {
            return Err(not_found!("quota for `{}` not found", input.tenant).into());
        }
        (Some(bytes), expected) => {
            let record: QuotaRecord = serde_json::from_slice(bytes)
                .map_err(|err| server_error!("parsing stored quota: {err}"))?;
            if record.version != expected {
                return Err(QuotaError::Conflict {
                    tenant: input.tenant,
                    current_version: Some(record.version),
                });
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
        serde_json::to_vec(&next).map_err(|err| server_error!("serializing quota: {err}"))?;

    match StateStore::cas(provider, &key, expected_bytes, &next_bytes).await {
        Ok(()) => Ok(next),
        Err(CasError::Conflict(observed)) => {
            let current_version = observed
                .as_deref()
                .and_then(|bytes| serde_json::from_slice::<QuotaRecord>(bytes).ok())
                .map(|record| record.version);
            Err(QuotaError::Conflict {
                tenant: input.tenant,
                current_version,
            })
        }
        Err(CasError::Store(err)) => Err(server_error!("{err}").into()),
    }
}
