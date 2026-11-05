//! # Root-package guest
//!
//! An Omnia WASI guest: every stem's routes merged onto one HTTP router and
//! every stem's topics merged onto one messaging router, over one provider.
//! Only the WASI exports are `wasm32`-gated: the routers are generic over
//! their provider, so `tests/routes.rs` and `tests/messaging.rs` build the
//! production routers natively under `omnia_test::guest::Provider` doubles.
//!
//! This root-package layout (`src/lib.rs`) is the compiling reference for
//! new Omnia services and the shape the guest template seeds.

use common::{Capabilities, config};
use omnia_sdk::api::{Client, http, messaging};

/// Build the HTTP router over one provider-owning [`Client`].
///
/// Omnia creates one component instance per request, so the router and the
/// client are built inside each call. The list holds one `<stem>::router(),`
/// per stem, each added at the end, in the array form a stem lists its parts
/// in. Merging panics where two stems serve one path under one method, so
/// the rung under `tests/` holds the list to one route apiece.
pub fn http_router<P: Capabilities>(provider: P) -> axum::Router {
    common::routers([
        // one router() per stem, each added at the end
        capability::router(),
        docstore::router(),
        gtfs_adapter::router(),
        pattern::router(),
        pulse_adapter::router(),
        pulse_connector::router(),
        sql::router(),
        tally_connector::router(),
    ])
    .with_state(Client::new(provider))
}

/// Build the exact-topic messaging router over one provider-owning [`Client`].
///
/// Each stem names its topics by bare suffix, and `map_topics` qualifies
/// them all for the deployment's environment, so a delivery from another
/// environment is unhandled rather than consumed. The list holds one
/// `<stem>::topics(),` per stem, in the form [`http_router`] describes.
/// Merging panics where two stems consume one topic, so the rung under
/// `tests/` holds the list to one consumer apiece.
pub async fn topic_router<P: Capabilities>(provider: P) -> messaging::Router<P> {
    let env = config::env(&provider).await;
    common::topics([
        // one topics() per stem, each added at the end
        capability::topics(),
        docstore::topics(),
        gtfs_adapter::topics(),
        pattern::topics(),
        pulse_adapter::topics(),
        pulse_connector::topics(),
        sql::topics(),
        tally_connector::topics(),
    ])
    .map_topics(|topic| config::topic_for(&env, topic))
    .with_client(Client::new(provider))
}

http::export!(http_router);
messaging::export!(topic_router);
