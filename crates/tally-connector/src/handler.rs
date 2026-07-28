//! Tally APC connector handler.

use anyhow::Context as _;
use omnia_guest::api::{CallContext, Operation, Provider};
use omnia_guest::{Config, Error, Message, Publish, Result};
use serde::{Deserialize, Serialize};

use crate::TallyMessage;

const TALLY_TOPIC: &str = "realtime-tally-apc.v2";

impl<P> Operation<P> for TallyRequest
where
    P: Provider + Config + Publish,
{
    type Error = Error;
    type Input = Self;
    type Output = TallyReply;

    async fn call(input: Self, context: CallContext<'_, P>) -> Result<TallyReply> {
        let provider = context.provider;
        let message = &input.message;

        // forward to the tally APC topic
        let msg_vec = serde_json::to_vec(message).context("failed to serialize TallyMessage")?;
        let mut msg = Message::new(&msg_vec);
        let site = message.device.as_ref().map_or_else(|| "undefined", |device| &device.site);
        msg.headers.insert("key".to_string(), site.to_string());

        let env = Config::get(provider, "ENV").await.unwrap_or_else(|_| "dev".to_string());
        let topic = format!("{env}-{TALLY_TOPIC}");

        Publish::send(provider, &topic, &msg).await?;

        Ok(TallyReply("OK"))
    }
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
