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

use omnia_sdk::Capabilities;
use omnia_sdk::api::Handlers;

/// The stem's handlers, merged onto the root routers.
pub fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::from([
        blob::handlers(),
        broadcast::handlers(),
        document::handlers(),
        table::handlers(),
    ])
}
