//! # Capability examples
//!
//! Compiling proof for the Omnia guest capabilities the transit crates do
//! not otherwise exercise: [`BlobStore`], [`Broadcast`], [`DocumentStore`],
//! and [`TableStore`]. Each module carries one small, deliberately
//! domain-free [`Handler`] over its capability trait; the crate-level
//! tests drive every handler through `omnia_test::guest::Provider` doubles.
//! Route constants for mounting the handlers under `/examples/*` live in
//! [`routes`], and [`router`] serves all four for the workspace-root guest
//! to merge.
//!
//! [`Handler`]: omnia_sdk::api::Handler
//! [`BlobStore`]: omnia_sdk::BlobStore
//! [`Broadcast`]: omnia_sdk::Broadcast
//! [`DocumentStore`]: omnia_sdk::DocumentStore
//! [`TableStore`]: omnia_sdk::TableStore

pub mod blob;
pub mod broadcast;
pub mod document;
pub mod routes;
pub mod table;

pub use blob::{ArchiveReply, ArchiveRequest, archive};
pub use broadcast::{AlertReply, AlertRequest, alert};
pub use document::{NoteReply, NoteRequest, note};
use omnia_sdk::api::Client;
use omnia_sdk::api::http::post;
use omnia_sdk::{BlobStore, Broadcast, DocumentStore, TableStore};
pub use table::{ReadingReply, ReadingRequest, reading};

/// The crate's routes, merged onto the guest's router: one body-JSON POST per handler.
pub fn router<P>() -> axum::Router<Client<P>>
where
    P: BlobStore + Broadcast + DocumentStore + TableStore + Send + Sync + 'static,
{
    axum::Router::new()
        .route(routes::ARCHIVE, post(archive))
        .route(routes::ALERT, post(alert))
        .route(routes::NOTE, post(note))
        .route(routes::READING, post(reading))
}
