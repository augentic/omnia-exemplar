//! Messaging rung: the production topic table driven natively.
//!
//! `messaging_router` is a guest-side value, so one delivery per subscribed
//! topic goes straight to `Router::handle` under `omnia_test::guest::Provider`
//! doubles. `routed` checks wiring, not semantics: every topic of the table
//! is registered under the environment's prefix. Business logic is covered
//! by each stem's own tests.

use common::config;
use omnia_sdk::api::messaging::{Delivery, DeliveryError};
use omnia_test::guest::{MapConfig, Provider};

/// The environment the doubles answer `ENV` with.
const ENV: &str = "test";

/// Every topic suffix the messaging router registers, one entry per topic
/// of `src/messaging.rs`, over the constants of `common::routes::topic` spelled in full.
const TOPICS: &[&str] = &[
    // one entry per topic, each added at the end
];

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
async fn routed() {
    let router = guest::messaging_router(provider()).await;
    let mut unrouted = Vec::new();
    for suffix in TOPICS {
        let topic = config::topic_for(ENV, suffix);
        let outcome = router.handle(delivery(topic.clone())).await;
        if matches!(outcome, Err(DeliveryError::UnhandledTopic(_))) {
            unrouted.push(topic);
        }
    }
    assert!(unrouted.is_empty(), "topics not routed:\n{}", unrouted.join("\n"));
}

#[tokio::test]
async fn another_environment_is_unhandled() {
    let router = guest::messaging_router(provider()).await;
    for suffix in TOPICS {
        let topic = config::topic_for("other", suffix);
        let outcome = router.handle(delivery(topic.clone())).await;
        assert!(matches!(outcome, Err(DeliveryError::UnhandledTopic(_))), "`{topic}` was consumed");
    }
}
