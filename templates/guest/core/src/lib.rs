//! # guest
//!
//! An Omnia WASI guest: every stem's routes merged onto one HTTP router and
//! every stem's topics merged onto one messaging router, over one provider.
//! Only the WASI exports are `wasm32`-gated: the routers are generic over
//! their provider, so `tests/routes.rs` and `tests/messaging.rs` build the
//! production routers natively under `omnia_test::guest::Provider` doubles.

use common::{Capabilities, config};
use omnia_sdk::api::{Client, http, messaging};

/// Build the HTTP router over one provider-owning [`Client`].
///
/// Omnia creates one component instance per request, so the router and the
/// client are built inside each call. One statement per stem between
/// `Router::new()` and `with_state`, each added at the end of the list:
/// `let router = router.merge(<stem>::router());`. The list is `let`
/// statements rather than a method chain because rustfmt folds a short chain
/// onto one line, and the scaffold adds a stem by inserting its line after
/// the first. Merging panics where two stems serve one path under one
/// method, so the rung under `tests/` holds the list to one route apiece.
pub fn http_router<P: Capabilities>(provider: P) -> axum::Router {
    let router = axum::Router::new();
    router.with_state(Client::new(provider))
}

/// Build the exact-topic messaging router over one provider-owning [`Client`].
///
/// Each stem names its topics by bare suffix, and `map_topics` qualifies
/// them all for the deployment's environment, so a delivery from another
/// environment is unhandled rather than consumed. One statement per stem
/// between `Routes::new()` and `map_topics`, in the form [`http_router`]
/// describes: `let routes = routes.merge(<stem>::topics());`. Merging panics
/// where two stems consume one topic, so the rung under `tests/` holds the
/// list to one consumer apiece.
pub async fn topic_router<P: Capabilities>(provider: P) -> messaging::Router<P> {
    let env = config::env(&provider).await;
    let routes = messaging::Routes::new();
    let routes = routes.map_topics(|topic| config::topic_for(&env, topic));
    messaging::Router::new(Client::new(provider)).merge(routes)
}

http::export!(http_router);
messaging::export!(topic_router);
