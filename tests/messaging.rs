//! Messaging rung: the production messaging router built natively.
//!
//! Building the router is the check: it panics where two stems consume one
//! topic. A stem's topics are driven by its own suites through its
//! component; this rung holds the merged whole.

use omnia_sdk::messaging::Delivery;

#[tokio::test]
async fn registers() {
    let router = guest::router();

    let error = router.handle(Delivery::default()).await.expect_err("no topic");

    assert_eq!(error, omnia_sdk::Error::bad_request("missing-topic", "the delivery has no topic"));
}

#[tokio::test]
async fn unknown_topic() {
    let router = guest::router();

    let error = router.handle(Delivery::new("nothing-here", &[])).await.expect_err("no route");

    assert_eq!(
        error,
        omnia_sdk::Error::not_found("unhandled-topic", "no route for topic `nothing-here`")
    );
}
