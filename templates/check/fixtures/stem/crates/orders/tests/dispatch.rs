//! Consuming XML dispatch notices through `topic_with`.

use omnia_sdk::api::Client;
use omnia_test::guest::{Provider, deliver};

#[tokio::test]
async fn xml_notice() {
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
        provider.storage.state("orders.dispatched.ord-7").as_deref(),
        Some(b"NZP".as_slice())
    );
}
