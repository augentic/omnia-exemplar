//! # Orders
//!
//! The fixture stem that proves a generated stem compiles and passes its
//! tests inside a rendered template scaffold.

pub mod catalogue;
pub mod dispatch;
pub mod lookup;
pub mod placing;

use omnia_sdk::api::Handlers;
use omnia_sdk::{Config, Publisher, SqlDb, StateStore};

/// The capabilities the stem's handlers reach, implemented by every provider that has them.
pub trait Requires: Config + Publisher + SqlDb + StateStore + Send + Sync + 'static {}

impl<P> Requires for P where P: Config + Publisher + SqlDb + StateStore + Send + Sync + 'static {}

/// The stem's handlers, merged onto the root routers.
pub fn handlers<P: Requires>() -> Handlers<P> {
    Handlers::from([
        catalogue::handlers(),
        dispatch::handlers(),
        lookup::handlers(),
        placing::handlers(),
    ])
}
