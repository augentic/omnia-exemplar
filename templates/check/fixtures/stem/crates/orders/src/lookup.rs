//! Looking up an order by id, answering `202 Accepted` while it is pending
//! and `409 Conflict` when its state is locked.

use axum::Json;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use common::Capabilities;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::{StateStore, not_found, server_error};
use serde::{Deserialize, Serialize};

// The paths the part serves.
pub(crate) const ORDER: &str = "/orders/{id}";

pub(crate) fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::new().route(ORDER, get(lookup))
}

pub(crate) fn order_key(id: &str) -> String {
    format!("orders/order/{id}")
}

/// Stored state of an order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderStatus {
    /// Order identifier.
    pub id: String,
    /// Whether the order has finished settling.
    pub settled: bool,
    /// Whether a concurrent operation holds a lock on the order.
    #[serde(default)]
    pub locked: bool,
}

/// Why an order lookup was refused.
#[derive(Debug, thiserror::Error)]
pub enum LookupError {
    /// Another operation holds a lock on the order.
    #[error("order `{0}` is locked")]
    Conflict(String),
    /// The store or a precondition failed.
    #[error(transparent)]
    Host(#[from] omnia_sdk::Error),
}

impl From<anyhow::Error> for LookupError {
    fn from(error: anyhow::Error) -> Self {
        Self::Host(error.into())
    }
}

impl IntoResponse for LookupError {
    fn into_response(self) -> Response {
        match self {
            Self::Conflict(ref id) => {
                let body = serde_json::json!({
                    "error": "conflict",
                    "message": self.to_string(),
                    "id": id,
                });
                (StatusCode::CONFLICT, Json(body)).into_response()
            }
            Self::Host(inner) => inner.into_response(),
        }
    }
}

// `202 Accepted` while the order is still settling.
pub(crate) async fn lookup<P>(
    context: Context<P>, Path(id): Path<String>,
) -> Result<(StatusCode, Json<OrderStatus>), LookupError>
where
    P: StateStore + 'static,
{
    let key = order_key(&id);
    let bytes = StateStore::get(context.provider(), &key)
        .await?
        .ok_or_else(|| not_found!("order `{id}` not found"))?;
    let status: OrderStatus =
        serde_json::from_slice(&bytes).map_err(|err| server_error!("order record: {err}"))?;
    if status.locked {
        return Err(LookupError::Conflict(id));
    }
    let code = if status.settled { StatusCode::OK } else { StatusCode::ACCEPTED };
    Ok((code, Json(status)))
}
