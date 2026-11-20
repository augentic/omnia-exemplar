//! # Capability examples
//!
//! Compiling proof for the Omnia host interfaces the transit crates do not
//! otherwise exercise: [`blobstore`], [`websocket`], [`docstore`], and
//! [`sql`]. Each module carries one small, deliberately domain-free handler
//! over its interface; the crate-level tests drive every handler through
//! the stem's component over `omnia_test::host` backends.
//!
//! [`blobstore`]: omnia_sdk::blobstore
//! [`websocket`]: omnia_sdk::websocket
//! [`docstore`]: omnia_sdk::docstore
//! [`sql`]: omnia_sdk::sql

pub mod blob;
pub mod broadcast;
pub mod document;
pub mod table;

use omnia_sdk::Handlers;

/// The stem's handlers, merged onto the root router.
pub fn handlers() -> Handlers {
    Handlers::from([
        blob::handlers(),
        broadcast::handlers(),
        document::handlers(),
        table::handlers(),
    ])
}
