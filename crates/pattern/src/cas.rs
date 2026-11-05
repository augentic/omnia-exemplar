//! Optimistic-concurrency update over [`StateStore::cas`].
//!
//! Demonstrates compare-and-swap state updates where a concurrent write is
//! rejected with `409 Conflict`. Because `omnia_sdk::Error` covers only
//! `400`, `404`, `500`, and `502`, the handler answers `Result<Json<QuotaRecord>,
//! QuotaError>` and maps [`CasError::Conflict`] to [`StatusCode::CONFLICT`]
//! in [`QuotaError`]'s [`IntoResponse`] implementation.

use axum::Json;
use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use axum::routing::put;
use common::Capabilities;
use http::StatusCode;
use omnia_sdk::api::Context;
use omnia_sdk::api::http::Router;
use omnia_sdk::{CasError, StateStore, not_found, server_error};
use serde::{Deserialize, Serialize};

pub(crate) const QUOTA: &str = "/examples/patterns/quotas/{tenant}";

pub(crate) fn routes<P: Capabilities>() -> Router<P> {
    Router::new().route(QUOTA, put(update_quota))
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

// The conflict body is omnia's `ErrorBody` pair plus the domain fields; a
// host failure is the framework envelope at its own status.
impl IntoResponse for QuotaError {
    fn into_response(self) -> Response {
        match self {
            Self::Conflict {
                ref tenant,
                current_version,
            } => {
                let body = serde_json::json!({
                    "error": "conflict",
                    "message": self.to_string(),
                    "tenant": tenant,
                    "current_version": current_version,
                });
                (StatusCode::CONFLICT, Json(body)).into_response()
            }
            Self::Host(inner) => inner.into_response(),
        }
    }
}

pub(crate) async fn update_quota<P>(
    context: Context<P>, Path(tenant): Path<String>, Json(input): Json<UpdateQuotaRequest>,
) -> Result<Json<QuotaRecord>, QuotaError>
where
    P: StateStore + 'static,
{
    let provider = context.provider();
    let key = quota_key(&tenant);
    let current = StateStore::get(provider, &key).await?;

    let expected_bytes = match (&current, input.expected_version) {
        (None, 0) => None,
        (None, _) => {
            return Err(not_found!("quota for `{tenant}` not found").into());
        }
        (Some(bytes), expected) => {
            let record: QuotaRecord = serde_json::from_slice(bytes)
                .map_err(|err| server_error!("parsing stored quota: {err}"))?;
            if record.version != expected {
                return Err(QuotaError::Conflict {
                    tenant,
                    current_version: Some(record.version),
                });
            }
            Some(bytes.as_slice())
        }
    };

    let next = QuotaRecord {
        tenant: tenant.clone(),
        version: input.expected_version + 1,
        limit: input.limit,
    };
    let next_bytes =
        serde_json::to_vec(&next).map_err(|err| server_error!("serializing quota: {err}"))?;

    match StateStore::cas(provider, &key, expected_bytes, &next_bytes).await {
        Ok(()) => Ok(Json(next)),
        Err(CasError::Conflict(observed)) => {
            let current_version = observed
                .as_deref()
                .and_then(|bytes| serde_json::from_slice::<QuotaRecord>(bytes).ok())
                .map(|record| record.version);
            Err(QuotaError::Conflict {
                tenant,
                current_version,
            })
        }
        Err(CasError::Store(err)) => Err(server_error!("{err}").into()),
    }
}
