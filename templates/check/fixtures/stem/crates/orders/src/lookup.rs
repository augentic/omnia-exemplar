//! Looking up an order by id, answering `202 Accepted` while it is pending
//! and `409 Conflict` when its state is locked.

use axum::Json;
use axum::response::{IntoResponse, Response};
use common::Capabilities;
use http::{HeaderValue, StatusCode};
use omnia_sdk::api::http::{MethodFilter, RawRequest, handle_with};
use omnia_sdk::api::{Client, Context, DecodeError};
use omnia_sdk::{HttpError, StateStore, not_found, server_error};
use serde::{Deserialize, Serialize};

// The paths the part serves.
pub(crate) const ORDER: &str = "/orders/{id}";

pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {
    axum::Router::new().route(
        ORDER,
        handle_with(
            MethodFilter::GET,
            lookup,
            |raw: RawRequest<'_>| decode_lookup(raw.path_params),
            encode_lookup,
        ),
    )
}

pub(crate) fn order_key(id: &str) -> String {
    format!("orders/order/{id}")
}

fn decode_lookup(params: &[(String, String)]) -> Result<LookupOrder, DecodeError> {
    let id = params
        .iter()
        .find_map(|(key, value)| (key == "id").then(|| value.clone()))
        .ok_or_else(|| DecodeError::new("missing `id` path parameter"))?;
    Ok(LookupOrder { id })
}

fn encode_lookup(reply: OrderStatus) -> Response {
    let status = if reply.settled { StatusCode::OK } else { StatusCode::ACCEPTED };
    (status, Json(reply)).into_response()
}

/// Request for an order's current status.
#[derive(Debug, Clone, Deserialize)]
pub struct LookupOrder {
    /// Order identifier (path parameter).
    pub id: String,
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

impl From<LookupError> for HttpError {
    fn from(error: LookupError) -> Self {
        match error {
            LookupError::Conflict(ref id) => {
                let body = serde_json::json!({
                    "error": "conflict",
                    "message": error.to_string(),
                    "id": id,
                });
                Self::with_body(
                    StatusCode::CONFLICT,
                    HeaderValue::from_static("application/json"),
                    body.to_string().into_bytes(),
                )
            }
            LookupError::Host(inner) => inner.into(),
        }
    }
}

pub(crate) async fn lookup<P>(
    input: LookupOrder, context: Context<P>,
) -> Result<OrderStatus, LookupError>
where
    P: StateStore,
{
    let key = order_key(&input.id);
    let bytes = StateStore::get(context.provider(), &key)
        .await?
        .ok_or_else(|| not_found!("order `{}` not found", input.id))?;
    let status: OrderStatus =
        serde_json::from_slice(&bytes).map_err(|err| server_error!("order record: {err}"))?;
    if status.locked {
        return Err(LookupError::Conflict(input.id));
    }
    Ok(status)
}
