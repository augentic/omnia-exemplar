//! # Capability examples
//!
//! Compiling proof for the Omnia guest capabilities the transit crates do
//! not otherwise exercise: [`BlobStore`], [`Broadcast`], [`DocumentStore`],
//! and [`TableStore`]. Each module carries one small, deliberately
//! domain-free [`Handler`] over its capability trait; the crate-level
//! tests drive every handler through `omnia_test::provider!` doubles. Route
//! constants for mounting the handlers under `/examples/*` live in
//! [`routes`]; the workspace-root guest wires all four.
//!
//! [`Handler`]: omnia_guest::api::Handler
//! [`BlobStore`]: omnia_guest::BlobStore
//! [`Broadcast`]: omnia_guest::Broadcast
//! [`DocumentStore`]: omnia_guest::DocumentStore
//! [`TableStore`]: omnia_guest::TableStore

pub mod blob;
pub mod broadcast;
pub mod document;
pub mod routes;
pub mod table;

pub use blob::{ArchiveReply, ArchiveRequest, archive};
pub use broadcast::{AlertReply, AlertRequest, alert};
pub use document::{NoteReply, NoteRequest, note};
pub use table::{ReadingReply, ReadingRequest, reading};
