//! Recording XML dispatch notices from the warehouse stream.

use common::{Capabilities, topic};
use omnia_sdk::api::messaging::Delivery;
use omnia_sdk::api::{Context, DecodeError, Handlers};
use omnia_sdk::{Result, StateStore, bad_request};
use serde::{Deserialize, Serialize};

pub(crate) fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::new().topic_with(topic::ORDER_DISPATCHED, on_dispatched, decode_dispatch_xml)
}

pub(crate) fn dispatched_key(id: &str) -> String {
    format!("orders/dispatched/{id}")
}

fn decode_dispatch_xml(delivery: &Delivery) -> std::result::Result<Dispatched, DecodeError> {
    quick_xml::de::from_reader(delivery.payload.as_slice())
        .map_err(|error| DecodeError::new(format!("malformed dispatch XML: {error}")))
}

/// An XML dispatch notice (`<Dispatched><id>…</id><carrier>…</carrier></Dispatched>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dispatched {
    /// Order identifier.
    pub id: String,
    /// Carrier code.
    pub carrier: String,
}

pub(crate) async fn on_dispatched<P>(context: Context<P>, input: Dispatched) -> Result<()>
where
    P: StateStore,
{
    if input.carrier.is_empty() {
        return Err(bad_request!("dispatch notice lacks a carrier"));
    }
    let key = dispatched_key(&input.id);
    context.state().set(&key, input.carrier.as_bytes(), None).await?;
    Ok(())
}
