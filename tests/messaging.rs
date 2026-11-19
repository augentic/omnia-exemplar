//! Messaging rung: the production messaging router built natively.
//!
//! Building the router is the check: it panics where two stems consume one
//! topic. A stem's topics are driven by its own suites through its
//! component; this rung holds the merged whole.

use omnia_sdk::api::messaging::Delivery;

#[tokio::test]
async fn registers() {
    let router = guest::router();

    let outcome = router.handle(Delivery::default()).await;

    assert_eq!(
        outcome,
        Err(omnia_sdk::Error::bad_request("missing-topic", "the delivery has no topic"))
    );
}

#[tokio::test]
async fn unknown_topic() {
    let router = guest::router();

    let outcome = router.handle(Delivery::new("nothing-here", &[])).await;

    assert_eq!(
        outcome,
        Err(omnia_sdk::Error::not_found("unhandled-topic", "no route for topic `nothing-here`"))
    );
}
