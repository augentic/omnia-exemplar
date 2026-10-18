//! Placing an order within the deployment's line limit, and hearing it confirmed.

use common::{Capabilities, config, topic};
use omnia_sdk::api::http::post;
use omnia_sdk::api::{Client, Context, messaging};
use omnia_sdk::{Config, Message, Publish, Result, StateStore, bad_request, server_error};
use serde::{Deserialize, Serialize};

// The settings the part reads, each key its own name.
pub(crate) const ORDERS_LIMIT: &str = "ORDERS_LIMIT";

// The paths the part serves.
pub(crate) const ORDERS: &str = "/orders";

// The state key of the running count of lines placed.
pub(crate) const LINES_PLACED: &str = "orders/lines-placed";

pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {
    axum::Router::new().route(ORDERS, post(place))
}

pub(crate) fn topics<P: Capabilities>(
    router: messaging::Router<P>, env: &str,
) -> messaging::Router<P> {
    router.route(config::topic_for(env, topic::ORDER_PLACED), messaging::consume(on_placed))
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

pub(crate) async fn place<P>(input: PlaceOrder, context: Context<P>) -> Result<Placed>
where
    P: Config + Publish,
{
    let provider = context.provider();
    let limit: usize = Config::get(provider, ORDERS_LIMIT)
        .await?
        .parse()
        .map_err(|error| server_error!("`{ORDERS_LIMIT}` is not a count: {error}"))?;
    if input.lines.is_empty() {
        return Err(bad_request!("an order holds at least one line"));
    }
    if input.lines.len() > limit {
        return Err(bad_request!("an order holds at most {limit} lines"));
    }

    let placed = Placed {
        lines: input.lines.len(),
    };
    let payload = serde_json::to_vec(&placed).map_err(|error| server_error!("{error}"))?;
    let topic = config::topic(provider, topic::ORDER_PLACED).await;
    Publish::send(provider, &topic, &Message::new(&payload)).await?;
    Ok(placed)
}

pub(crate) async fn on_placed<P>(input: Placed, context: Context<P>) -> Result<()>
where
    P: StateStore,
{
    let delta = i64::try_from(input.lines).map_err(|error| bad_request!("{error}"))?;
    let total = StateStore::increment(context.provider(), LINES_PLACED, delta).await?;
    tracing::info!(lines = input.lines, total, "order placed");
    Ok(())
}
