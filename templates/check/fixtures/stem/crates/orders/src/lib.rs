//! # Orders
//!
//! The fixture stem that proves a generated stem compiles and passes its
//! tests inside a rendered template scaffold.

pub mod catalogue;
pub mod dispatch;
pub mod lookup;
pub mod placing;

use omnia_sdk::Handlers;

/// The stem's handlers, merged onto the root router.
pub fn handlers() -> Handlers {
    Handlers::from([
        catalogue::handlers(),
        dispatch::handlers(),
        lookup::handlers(),
        placing::handlers(),
    ])
}
