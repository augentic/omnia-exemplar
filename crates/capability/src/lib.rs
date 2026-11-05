//! # Capability examples
//!
//! Compiling proof for the Omnia guest capabilities the transit crates do
//! not otherwise exercise: [`BlobStore`], [`Broadcast`], [`DocumentStore`],
//! and [`TableStore`]. Each module carries one small, deliberately
//! domain-free handler over its capability trait; the crate-level tests
//! drive every handler through `omnia_test::guest::Provider` doubles.
//!
//! [`BlobStore`]: omnia_sdk::BlobStore
//! [`Broadcast`]: omnia_sdk::Broadcast
//! [`DocumentStore`]: omnia_sdk::DocumentStore
//! [`TableStore`]: omnia_sdk::TableStore

pub mod blob;
pub mod broadcast;
pub mod document;
pub mod table;

use common::Capabilities;
use omnia_sdk::api::Client;

/// The stem's routes, merged onto the root router.
pub fn router<P: Capabilities>() -> axum::Router<Client<P>> {
    common::routers([
        // one entry per part with routes, each added at the end
        blob::routes(),
        broadcast::routes(),
        document::routes(),
        table::routes(),
    ])
}
