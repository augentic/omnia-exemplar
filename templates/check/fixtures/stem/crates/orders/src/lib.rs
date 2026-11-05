//! # orders
//!
//! The `orders` stem.

pub mod catalogue;
pub mod dispatch;
pub mod lookup;
pub mod placing;

use common::Capabilities;
use omnia_sdk::api::{Client, messaging};

/// The stem's routes, merged onto the root router.
pub fn router<P: Capabilities>() -> axum::Router<Client<P>> {
    common::routers([
        // one entry per part with routes, each added at the end
        catalogue::routes(),
        lookup::routes(),
        placing::routes(),
    ])
}

/// The stem's topics, registered on the root messaging router under `env`.
#[must_use]
pub fn topics<P: Capabilities>(router: messaging::Router<P>, env: &str) -> messaging::Router<P> {
    common::topics(router, env, [
        // one entry per part with topics, each added at the end
        dispatch::topics,
        placing::topics,
    ])
}
