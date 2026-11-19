//! # guest
//!
//! An Omnia WASI guest: every stem's handlers merged onto one router. A
//! handler reaches the host through `omnia-sdk`'s bindings, so building and
//! routing need no host: `tests/routes.rs` and `tests/messaging.rs` build
//! the production router natively.

use omnia_sdk::{Handlers, Router, http, messaging};

/// Build the guest router.
///
/// Omnia creates one component instance per request or delivery, so the
/// router is built inside each call. The handler list holds one
/// `<stem>::handlers(),` per stem, each added at the end, in the array form
/// a stem lists its parts in. The comment heading the array keeps it one
/// entry per line, so two stems added at once merge as two lines.
///
/// Merging panics where two stems serve one path under one method or
/// consume one topic, so the rungs under `tests/` hold the list to one
/// handler apiece.
pub fn router() -> Router {
    Handlers::from([
        // one `<stem>::handlers(),` per stem, each added at the end
    ])
    .into_router()
}

http::export!(router);
messaging::export!(router);
