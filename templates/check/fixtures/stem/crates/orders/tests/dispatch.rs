//! Consuming XML dispatch notices through `consume_with`.

use omnia_sdk::api::Client;
use omnia_sdk::api::messaging::{Delivery, DeliveryError, Router};
use omnia_test::guest::Provider;

#[tokio::test]
async fn records_carrier_from_xml() {
    let provider = Provider::default();
    let router = orders::topics(Router::new(Client::new("test", provider.clone())), "test");
    let delivery = Delivery {
        topic: Some("test-order-dispatched".to_string()),
        payload: b"<Dispatched><id>ord-7</id><carrier>NZP</carrier></Dispatched>".to_vec(),
        ..Delivery::default()
    };

    router.handle(delivery).await.expect("dispatch notice handled");

    assert_eq!(
        provider.storage.state("orders/dispatched/ord-7").as_deref(),
        Some(b"NZP".as_slice())
    );
}

#[tokio::test]
async fn rejects_malformed_xml() {
    let provider = Provider::default();
    let router = orders::topics(Router::new(Client::new("test", provider.clone())), "test");
    let delivery = Delivery {
        topic: Some("test-order-dispatched".to_string()),
        payload: b"<Dispatched><unclosed>".to_vec(),
        ..Delivery::default()
    };

    let error = router.handle(delivery).await.expect_err("malformed XML rejected");

    assert!(matches!(error, DeliveryError::Rejected(_)), "{error}");
    assert!(provider.storage.is_empty());
}
