//! Tally APC connector handler.

use axum::Json;
use axum::routing::post;
use common::topic;
use omnia_sdk::Result;
use omnia_sdk::api::Handlers;
use omnia_sdk::messaging::types::{Client, Message};
use omnia_sdk::messaging::{self, producer};
use serde::{Deserialize, Serialize};

use crate::types::TallyMessage;

pub(crate) const APC: &str = "/api/apc";

pub(crate) fn handlers() -> Handlers {
    Handlers::new().route(APC, post(tally))
}

pub(crate) async fn tally(Json(input): Json<TallyRequest>) -> Result<Json<TallyReply>> {
    let message = &input.message;

    // forward to the tally APC topic
    let site = message.device.as_ref().map_or_else(|| "undefined", |device| &device.site);
    let msg = Message::json(message)?.with_key(site);
    let client = Client::connect(messaging::DEFAULT.to_owned()).await?;
    producer::send(&client, topic::TALLY_APC.to_owned(), msg).await?;

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
