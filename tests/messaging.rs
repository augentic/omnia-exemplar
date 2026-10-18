//! Messaging rung: the production messaging router built natively.
//!
//! Building the router is the check: it panics where two crates consume one
//! topic. A crate's topics are driven by its own `dispatch` tests through
//! its `topics()`; this rung holds the registered whole.

use acme_common::config;
use omnia_sdk::api::messaging::{Delivery, DeliveryError};
use omnia_test::guest::{MapConfig, Provider};

/// The environment the doubles answer `ENV` with.
const ENV: &str = "dev";

fn provider() -> Provider {
    Provider::default().config(MapConfig::default().with([(config::ENV, ENV)]))
}

fn delivery(topic: String) -> Delivery {
    Delivery {
        topic: Some(topic),
        ..Delivery::default()
    }
}

#[tokio::test]
async fn registers() {
    let router = guest::messaging_router(provider()).await;

    let outcome = router.handle(Delivery::default()).await;

    assert_eq!(outcome, Err(DeliveryError::MissingTopic));
}

#[tokio::test]
async fn an_unknown_topic_is_unhandled() {
    let router = guest::messaging_router(provider()).await;
    let topic = config::topic_for(ENV, "nothing-here");

    let outcome = router.handle(delivery(topic.clone())).await;

    assert_eq!(outcome, Err(DeliveryError::UnhandledTopic(topic)));
}
