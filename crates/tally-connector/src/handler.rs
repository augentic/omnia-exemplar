//! Tally APC connector handler.

use acme_common::{config, routes};
use anyhow::Context as _;
use omnia_guest::api::Context;
use omnia_guest::{Config, Message, Publish, Result};
use serde::{Deserialize, Serialize};

use crate::TallyMessage;

#[omnia_guest::handler]
async fn tally_request<P>(input: TallyRequest, context: Context<'_, P>) -> Result<TallyReply>
where
    P: Config + Publish,
{
    let provider = context.provider;
    let message = &input.message;

    // forward to the tally APC topic
    let msg_vec = serde_json::to_vec(message).context("failed to serialize TallyMessage")?;
    let mut msg = Message::new(&msg_vec);
    let site = message.device.as_ref().map_or_else(|| "undefined", |device| &device.site);
    msg.headers.insert("key".to_string(), site.to_string());

    let topic = config::topic(provider, routes::topic::TALLY_APC).await;

    Publish::send(provider, &topic, &msg).await?;

    Ok(TallyReply("OK"))
}

/// Tally request
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct TallyRequest {
    /// Tally message
    pub message: TallyMessage,
}

/// Tally response.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct TallyReply(pub &'static str);
