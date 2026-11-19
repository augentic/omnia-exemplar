//! Looking up an order by id, answering `202 Accepted` while it is pending
//! and `409 Conflict` when its state is locked.

use axum::Json;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use omnia_sdk::api::Handlers;
use omnia_sdk::keyvalue::{self, store, types};
use omnia_sdk::not_found;
use serde::{Deserialize, Serialize};

// The paths the part serves.
pub(crate) const ORDER: &str = "/orders/{id}";

pub(crate) fn handlers() -> Handlers {
    Handlers::new().route(ORDER, get(lookup))
}

pub(crate) fn order_key(id: &str) -> String {
    format!("orders.order.{id}")
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

impl From<types::Error> for LookupError {
    fn from(error: types::Error) -> Self {
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
pub(crate) async fn lookup(
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<OrderStatus>), LookupError> {
    let state = store::open(keyvalue::DEFAULT.to_owned()).await?;
    let status: OrderStatus = state
        .get_json(&order_key(&id))
        .await?
        .ok_or_else(|| not_found!("order `{id}` not found"))?;
    if status.locked {
        return Err(LookupError::Conflict(id));
    }
    let code = if status.settled { StatusCode::OK } else { StatusCode::ACCEPTED };
    Ok((code, Json(status)))
}
