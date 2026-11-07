//! Consuming XML dispatch notices through `route_with`.

use omnia_sdk::api::Client;
use omnia_sdk::api::messaging::DeliveryError;
use omnia_test::guest::{Provider, deliver};

#[tokio::test]
async fn records_carrier_from_xml() {
    let provider = Provider::default();
    let router = orders::handlers().with_state(Client::new(provider.clone()));

    deliver(
        &router,
        "order-dispatched",
        b"<Dispatched><id>ord-7</id><carrier>NZP</carrier></Dispatched>",
    )
    .await
    .expect("dispatch notice handled");

    assert_eq!(
        provider.storage.state("orders/dispatched/ord-7").as_deref(),
        Some(b"NZP".as_slice())
    );
}

#[tokio::test]
async fn rejects_malformed_xml() {
    let provider = Provider::default();
    let router = orders::handlers().with_state(Client::new(provider.clone()));

    let error = deliver(&router, "order-dispatched", b"<Dispatched><unclosed>")
        .await
        .expect_err("malformed XML rejected");

    assert!(matches!(error, DeliveryError::Rejected(_)), "{error}");
    assert!(provider.storage.is_empty());
}
