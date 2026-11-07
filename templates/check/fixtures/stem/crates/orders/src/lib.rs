//! # Orders
//!
//! The fixture stem that proves a generated stem compiles and passes its
//! tests inside a rendered template scaffold.

pub mod catalogue;
pub mod dispatch;
pub mod lookup;
pub mod placing;

use omnia_sdk::Capabilities;
use omnia_sdk::api::Handlers;

/// The stem's handlers, merged onto the root routers.
pub fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::from([
        catalogue::handlers(),
        dispatch::handlers(),
        lookup::handlers(),
        placing::handlers(),
    ])
}
