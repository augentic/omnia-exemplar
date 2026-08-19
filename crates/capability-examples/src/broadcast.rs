//! Broadcast example: push an alert to connected WebSocket clients.
//!
//! [`Broadcast::send`] is client-side only — the guest connects out to the
//! broadcast channel — so serving this operation requires no WebSocket
//! export.

use omnia_guest::api::{CallContext, Provider};
use omnia_guest::{Broadcast, Result};
use serde::{Deserialize, Serialize};

/// Broadcast an alert to a channel.
#[derive(Debug, Clone, Deserialize)]
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

#[omnia_guest::operation]
async fn alert_request<P>(input: AlertRequest, context: CallContext<'_, P>) -> Result<AlertReply>
where
    P: Provider + Broadcast,
{
    Broadcast::send(context.provider, &input.channel, input.message.as_bytes(), input.sockets)
        .await?;
    Ok(AlertReply("OK"))
}
