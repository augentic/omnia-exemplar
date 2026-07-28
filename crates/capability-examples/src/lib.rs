//! # Capability examples
//!
//! Compiling proof for the Omnia guest capabilities the transit crates do
//! not otherwise exercise: [`BlobStore`], [`Broadcast`], [`DocumentStore`],
//! and [`TableStore`]. Each module carries one small, deliberately
//! domain-free [`Operation`] over its capability trait; the crate-level
//! tests drive every operation through an in-memory mock provider, and
//! `guests/typed` routes them so the default WASM capability
//! implementations are instantiated in a real guest.
//!
//! [`Operation`]: omnia_guest::api::Operation
//! [`BlobStore`]: omnia_guest::BlobStore
//! [`Broadcast`]: omnia_guest::Broadcast
//! [`DocumentStore`]: omnia_guest::DocumentStore
//! [`TableStore`]: omnia_guest::TableStore

pub mod blob;
pub mod broadcast;
pub mod document;
pub mod routes;
pub mod table;

pub use blob::{ArchiveReply, ArchiveRequest};
pub use broadcast::{AlertReply, AlertRequest};
pub use document::{NoteReply, NoteRequest};
pub use table::{ReadingReply, ReadingRequest};
