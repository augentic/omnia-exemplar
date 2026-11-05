//! # orders
//!
//! The `orders` stem.

pub mod catalogue;
pub mod dispatch;
pub mod lookup;
pub mod placing;

use common::Capabilities;
use omnia_sdk::api::{http, messaging};

/// The stem's routes, merged onto the root router.
pub fn router<P: Capabilities>() -> http::Routes<P> {
    common::routers([
        // one entry per part with routes, each added at the end
        catalogue::routes(),
        lookup::routes(),
        placing::routes(),
    ])
}

/// The stem's topics, merged onto the root messaging router.
pub fn topics<P: Capabilities>() -> messaging::Routes<P> {
    common::topics([
        // one entry per part with topics, each added at the end
        dispatch::topics(),
        placing::topics(),
    ])
}
