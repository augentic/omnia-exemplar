//! Consuming XML dispatch notices through `topic_with`.

mod support;

use omnia_test::host::{self, Backends};
use support::boot;

#[tokio::test]
async fn xml_notice() {
    let backends = Backends::defaults().await;
    let runtime = boot(backends.clone()).await;

    host::deliver(
        &runtime,
        "order-dispatched",
        b"<Dispatched><id>ord-7</id><carrier>NZP</carrier></Dispatched>",
    )
    .await
    .expect("dispatch notice handled");
    runtime.shutdown();

    assert_eq!(backends.state("orders.dispatched.ord-7").await.as_deref(), Some(b"NZP".as_slice()));
}
