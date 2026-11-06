//! Broadcast example: push an alert to connected WebSocket clients.
//!
//! [`Broadcast::send`] is client-side only — the guest connects out to the
//! broadcast channel — so serving this handler requires no WebSocket
//! export.

use axum::Json;
use axum::routing::post;
use common::Capabilities;
use omnia_sdk::api::Context;
use omnia_sdk::api::http::Routes;
use omnia_sdk::{Broadcast, Result};
use serde::{Deserialize, Serialize};

pub(crate) const ALERT: &str = "/examples/alert";

pub(crate) fn routes<P: Capabilities>() -> Routes<P> {
    Routes::new().route(ALERT, post(alert))
}

/// Broadcast an alert to a channel.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AlertRequest {
    /// Broadcast channel name.
    pub channel: String,
    /// Alert text sent to connected clients.
    pub message: String,
    /// Restrict delivery to these socket ids; `None` broadcasts to all.
    pub sockets: Option<Vec<String>>,
}

/// Broadcast acknowledgement.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct AlertReply(pub &'static str);

pub(crate) async fn alert<P>(
    context: Context<P>, Json(input): Json<AlertRequest>,
) -> Result<Json<AlertReply>>
where
    P: Broadcast + 'static,
{
    Broadcast::send(context.provider(), &input.channel, input.message.as_bytes(), input.sockets)
        .await?;
    Ok(Json(AlertReply("OK")))
}
