//! Messaging rung: the production messaging router built natively.
//!
//! Building the router is the check: it panics where two stems consume one
//! topic. A stem's topics are driven by its own tests through its
//! `handlers()`; this rung holds the merged whole, qualified for `ENV`.

use common::config;
use omnia_sdk::api::messaging::{Delivery, DeliveryError};
use omnia_test::guest::{MapConfig, Provider};

/// The environment the doubles answer `ENV` with.
const ENV: &str = "test";

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
    let router = guest::topic_router(provider()).await;

    let outcome = router.handle(Delivery::default()).await;

    assert_eq!(outcome, Err(DeliveryError::MissingTopic));
}

#[tokio::test]
async fn unknown_topic() {
    let router = guest::topic_router(provider()).await;
    let topic = config::topic_for(ENV, "nothing-here");

    let outcome = router.handle(delivery(topic.clone())).await;

    assert_eq!(outcome, Err(DeliveryError::UnhandledTopic(topic)));
}

#[tokio::test]
async fn every_topic_is_qualified() {
    let router = guest::topic_router(provider()).await;
    let prefix = config::topic_for(ENV, "");

    for topic in router.topics() {
        assert!(topic.starts_with(&prefix), "`{topic}` is not qualified for `{ENV}`");
    }
}
