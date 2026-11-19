//! Placing an order within the deployment's line limit, and hearing it confirmed.

use axum::Json;
use axum::routing::post;
use common::topic;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::keyvalue::{self, store};
use omnia_sdk::messaging::types::{Client, Message};
use omnia_sdk::messaging::{self, producer};
use omnia_sdk::{Result, Settings, bad_request};
use serde::{Deserialize, Serialize};

// The paths the part serves.
pub(crate) const ORDERS: &str = "/orders";

// The state key of the running count of lines placed.
pub(crate) const LINES_PLACED: &str = "orders.lines-placed";

pub(crate) fn handlers() -> Handlers {
    Handlers::new().route(ORDERS, post(place)).topic(topic::ORDER_PLACED, on_placed)
}

// The settings the part reads: `ORDERS_LIMIT`.
#[derive(Debug, Settings)]
#[setting(prefix = "ORDERS_")]
pub(crate) struct PlacingSettings {
    limit: usize,
}

/// An order to place.
#[derive(Debug, Clone, Deserialize)]
pub struct PlaceOrder {
    /// The lines of the order, at least one.
    pub lines: Vec<String>,
}

/// An order placed, as the topic carries it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Placed {
    /// How many lines the order holds.
    pub lines: usize,
}

pub(crate) async fn place(Json(input): Json<PlaceOrder>) -> Result<Json<Placed>> {
    let settings = PlacingSettings::load().await?;
    if input.lines.is_empty() {
        return Err(bad_request!("an order holds at least one line"));
    }
    if input.lines.len() > settings.limit {
        return Err(bad_request!("an order holds at most {} lines", settings.limit));
    }

    let placed = Placed {
        lines: input.lines.len(),
    };
    let client = Client::connect(messaging::DEFAULT.to_owned()).await?;
    producer::send(&client, topic::ORDER_PLACED.to_owned(), Message::json(&placed)?).await?;
    Ok(Json(placed))
}

pub(crate) async fn on_placed(_: Context, input: Placed) -> Result<()> {
    let delta = i64::try_from(input.lines).map_err(|error| bad_request!("{error}"))?;
    let state = store::open(keyvalue::DEFAULT.to_owned()).await?;
    let total = state.increment(LINES_PLACED.to_owned(), delta, None).await?;
    tracing::info!(lines = input.lines, total, "order placed");
    Ok(())
}
