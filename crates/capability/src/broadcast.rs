//! WebSocket example: push an alert to connected WebSocket clients.
//!
//! A [`websocket`] send is outbound only, so serving this handler requires
//! no WebSocket export.

use axum::Json;
use axum::routing::post;
use omnia_sdk::websocket::{self, Message};
use omnia_sdk::{Handlers, Result};
use serde::{Deserialize, Serialize};

pub(crate) const ALERT: &str = "/examples/alert";

pub(crate) fn handlers() -> Handlers {
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

pub(crate) async fn alert(Json(input): Json<AlertRequest>) -> Result<Json<AlertReply>> {
    let client = websocket::default().await?;
    let message = Message::Text(input.message);

    match input.sockets {
        Some(sockets) => {
            let sockets: Vec<&str> = sockets.iter().map(String::as_str).collect();
            client.send(&sockets, message).await?;
        }
        None => client.broadcast(message, &[]).await?,
    }
    Ok(Json(AlertReply("OK")))
}
