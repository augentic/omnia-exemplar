//! Messaging rung: the production messaging router built natively.
//!
//! Building the router is the check: it panics where two stems consume one
//! topic. A stem's topics are driven by its own tests through its
//! `handlers()`; this rung holds the merged whole.

use omnia_sdk::api::messaging::{Delivery, DeliveryError};
use omnia_test::guest::{Provider, deliver};

#[tokio::test]
async fn registers() {
    let router = guest::router(Provider::default());

    let outcome = router.handle(Delivery::default()).await;

    assert_eq!(outcome, Err(DeliveryError::MissingTopic));
}

#[tokio::test]
async fn unknown_topic() {
    let router = guest::router(Provider::default());

    let outcome = deliver(&router, "nothing-here", Vec::new()).await;

    assert_eq!(outcome, Err(DeliveryError::UnhandledTopic("nothing-here".to_owned())));
}
