//! Recording XML dispatch notices from the warehouse stream.

use common::topic;
use omnia_sdk::api::messaging::Delivery;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::keyvalue::types::Lifetime;
use omnia_sdk::keyvalue::{self, store};
use omnia_sdk::{Result, bad_request};
use serde::{Deserialize, Serialize};

pub(crate) fn handlers() -> Handlers {
    Handlers::new().topic_with(topic::ORDER_DISPATCHED, on_dispatched, decode_dispatch_xml)
}

pub(crate) fn dispatched_key(id: &str) -> String {
    format!("orders.dispatched.{id}")
}

fn decode_dispatch_xml(delivery: &Delivery) -> Result<Dispatched> {
    quick_xml::de::from_reader(&delivery.payload[..])
        .map_err(|error| bad_request!("malformed dispatch XML: {error}"))
}

/// An XML dispatch notice (`<Dispatched><id>…</id><carrier>…</carrier></Dispatched>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dispatched {
    /// Order identifier.
    pub id: String,
    /// Carrier code.
    pub carrier: String,
}

pub(crate) async fn on_dispatched(_: Context, input: Dispatched) -> Result<()> {
    if input.carrier.is_empty() {
        return Err(bad_request!("dispatch notice lacks a carrier"));
    }
    let state = store::open(keyvalue::DEFAULT.to_owned()).await?;
    state
        .set(dispatched_key(&input.id), input.carrier.into_bytes(), Lifetime::Forever, None)
        .await??;
    Ok(())
}
