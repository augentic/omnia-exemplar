//! WebSocket example: push an alert to connected WebSocket clients.
//!
//! A [`WebSockets`] send is outbound only, so serving this handler
//! requires no WebSocket export.

use axum::Json;
use axum::routing::post;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::{Result, WebSockets};
use serde::{Deserialize, Serialize};

pub(crate) const ALERT: &str = "/examples/alert";

pub(crate) fn handlers<P: crate::Requires>() -> Handlers<P> {
    Handlers::new().route(ALERT, post(alert))
}

/// Send an alert to connected WebSocket clients.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AlertRequest {
    /// Alert text sent to connected clients.
    pub message: String,
    /// Restrict delivery to these socket ids; `None` sends to every socket.
    pub sockets: Option<Vec<String>>,
}

/// Alert acknowledgement.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct AlertReply(pub &'static str);

pub(crate) async fn alert<P>(
    context: Context<P>, Json(input): Json<AlertRequest>,
) -> Result<Json<AlertReply>>
where
    P: WebSockets,
{
    let websockets = context.websockets();
    let data = input.message.as_bytes();

    match &input.sockets {
        Some(sockets) => {
            let sockets: Vec<&str> = sockets.iter().map(String::as_str).collect();
            websockets.send_to(&sockets, data).await?;
        }
        None => websockets.broadcast(data).await?,
    }
    Ok(Json(AlertReply("OK")))
}
