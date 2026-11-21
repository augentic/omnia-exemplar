//! Optimistic-concurrency update over [`Bucket::update_json`].
//!
//! Demonstrates a version-guarded state update where a concurrent write is
//! rejected with `409 Conflict`. [`Bucket::update_json`] retries a write that
//! raced another writer, and its closure checks the caller's expected version
//! against every record it reads, so a stale caller and a lost race both
//! answer [`conflict!`].
//!
//! [`Bucket::update_json`]: omnia_sdk::keyvalue::Bucket::update_json
//! [`conflict!`]: omnia_sdk::conflict

use axum::Json;
use axum::extract::Path;
use axum::routing::put;
use omnia_sdk::{Handlers, Result, bad_request, conflict, keyvalue, not_found};
use serde::{Deserialize, Serialize};

use crate::state_keys;

pub(crate) const QUOTA: &str = "/examples/patterns/quotas/{tenant}";

pub(crate) fn handlers() -> Handlers {
    Handlers::new().route(QUOTA, put(update_quota))
}

/// A versioned quota record stored in the `default` [`keyvalue`] bucket.
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

pub(crate) async fn update_quota(
    Path(tenant): Path<String>, Json(input): Json<UpdateQuotaRequest>,
) -> Result<Json<QuotaRecord>> {
    if !state_keys::is_portable(&tenant) {
        return Err(bad_request!("tenant `{tenant}` may hold only letters, digits, `_`, and `-`"));
    }

    let state = keyvalue::default().await?;
    let record =
        state
            .update_json(&state_keys::quota(&tenant), None, |current: Option<QuotaRecord>| {
                match current.map(|record| record.version) {
                    None if input.expected_version != 0 => {
                        Err(not_found!("quota for `{tenant}` not found"))
                    }
                    Some(version) if version != input.expected_version => Err(conflict!(
                        "quota for `{tenant}` changed concurrently (current version: {version})"
                    )),
                    _ => Ok(QuotaRecord {
                        tenant: tenant.clone(),
                        version: input.expected_version + 1,
                        limit: input.limit,
                    }),
                }
            })
            .await?;

    Ok(Json(record))
}
