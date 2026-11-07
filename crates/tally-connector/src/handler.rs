//! Tally APC connector handler.

use axum::Json;
use axum::routing::post;
use common::{Capabilities, topic};
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::{Message, Publish, Result};
use serde::{Deserialize, Serialize};

use crate::types::TallyMessage;

pub(crate) const APC: &str = "/api/apc";

pub(crate) fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::new().route(APC, post(tally))
}

pub(crate) async fn tally<P>(
    context: Context<P>, Json(input): Json<TallyRequest>,
) -> Result<Json<TallyReply>>
where
    P: Publish + 'static,
{
    let message = &input.message;

    // forward to the tally APC topic
    let site = message.device.as_ref().map_or_else(|| "undefined", |device| &device.site);
    let msg = Message::json(message)?.with_header("key", site);
    context.publish().send(topic::TALLY_APC, &msg).await?;

    Ok(Json(TallyReply("OK")))
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
