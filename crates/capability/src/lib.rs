//! # Capability examples
//!
//! Compiling proof for the Omnia guest capabilities the transit crates do
//! not otherwise exercise: [`BlobStore`], [`WebSockets`], [`DocumentDb`],
//! and [`SqlDb`]. Each module carries one small, deliberately
//! domain-free handler over its capability trait; the crate-level tests
//! drive every handler through `omnia_test::guest::Provider` doubles.

pub mod blob;
pub mod broadcast;
pub mod document;
pub mod table;

use omnia_sdk::api::Handlers;
use omnia_sdk::{BlobStore, DocumentDb, SqlDb, WebSockets};

/// The capabilities the stem's handlers reach, implemented by every provider that has them.
pub trait Requires: BlobStore + DocumentDb + SqlDb + WebSockets + Send + Sync + 'static {}

impl<P> Requires for P where P: BlobStore + DocumentDb + SqlDb + WebSockets + Send + Sync + 'static {}

/// The stem's handlers, merged onto the root routers.
pub fn handlers<P: Requires>() -> Handlers<P> {
    Handlers::from([
        blob::handlers(),
        broadcast::handlers(),
        document::handlers(),
        table::handlers(),
    ])
}
