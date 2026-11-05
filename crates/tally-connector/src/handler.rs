//! Tally APC connector handler.

use anyhow::Context as _;
use common::{Capabilities, config, topic};
use omnia_sdk::api::http::post;
use omnia_sdk::api::{Client, Context};
use omnia_sdk::{Config, Message, Publish, Result};
use serde::{Deserialize, Serialize};

use crate::TallyMessage;

/// Tally passenger-count (APC) ingress.
pub const APC: &str = "/api/apc";

pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {
    axum::Router::new().route(APC, post(tally))
}

/// Forwards a Tally APC message to the tally APC topic, keyed by device site.
///
/// # Errors
///
/// Returns an error if the message cannot be serialized or publishing fails.
pub async fn tally<P>(input: TallyRequest, context: Context<P>) -> Result<TallyReply>
where
    P: Config + Publish,
{
    let provider = context.provider();
    let message = &input.message;

    // forward to the tally APC topic
    let msg_vec = serde_json::to_vec(message).context("failed to serialize TallyMessage")?;
    let mut msg = Message::new(&msg_vec);
    let site = message.device.as_ref().map_or_else(|| "undefined", |device| &device.site);
    msg.headers.insert("key".to_string(), site.to_string());

    let topic = config::topic(provider, topic::TALLY_APC).await;

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
