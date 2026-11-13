//! Optimistic-concurrency update over [`StateStore::compare_and_swap`].
//!
//! Demonstrates compare-and-swap state updates where a concurrent write is
//! rejected with `409 Conflict`. Because `omnia_sdk::Error` covers only
//! `400`, `404`, `500`, and `502`, the handler answers `Result<Json<QuotaRecord>,
//! QuotaError>` and maps a swap's [`Conflict`] to [`StatusCode::CONFLICT`]
//! in [`QuotaError`]'s [`IntoResponse`] implementation.

use axum::Json;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::put;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::keyvalue::Conflict;
use omnia_sdk::{StateStore, bad_request, capability, not_found, server_error};
use serde::{Deserialize, Serialize};

use crate::state_keys;

pub(crate) const QUOTA: &str = "/examples/patterns/quotas/{tenant}";

pub(crate) fn handlers<P: crate::Requires>() -> Handlers<P> {
    Handlers::new().route(QUOTA, put(update_quota))
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

impl From<capability::Error> for QuotaError {
    fn from(error: capability::Error) -> Self {
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
    P: StateStore,
{
    if !state_keys::is_portable(&tenant) {
        return Err(
            bad_request!("tenant `{tenant}` may hold only letters, digits, `_`, and `-`").into()
        );
    }
    let key = state_keys::quota(&tenant);
    let current = context.state().get(&key).await?;

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
            Some(&bytes[..])
        }
    };

    let next = QuotaRecord {
        tenant: tenant.clone(),
        version: input.expected_version + 1,
        limit: input.limit,
    };
    let next_bytes =
        serde_json::to_vec(&next).map_err(|err| server_error!("serializing quota: {err}"))?;

    match context.state().compare_and_swap(&key, expected_bytes, &next_bytes, None).await? {
        Ok(()) => Ok(Json(next)),
        Err(Conflict { current }) => {
            let current_version = current
                .as_deref()
                .and_then(|bytes| serde_json::from_slice::<QuotaRecord>(bytes).ok())
                .map(|record| record.version);
            Err(QuotaError::Conflict {
                tenant,
                current_version,
            })
        }
    }
}
