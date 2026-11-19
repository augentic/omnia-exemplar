//! Document-store example: upsert a JSON note and read it back.

use anyhow::Context as _;
use axum::Json;
use axum::routing::post;
use omnia_sdk::{Handlers, Result, docstore};
use serde::{Deserialize, Serialize};

pub(crate) const NOTE: &str = "/examples/note";

pub(crate) fn handlers() -> Handlers {
    Handlers::new().route(NOTE, post(note))
}

/// Upsert a JSON note document.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NoteRequest {
    /// Document collection.
    pub store: String,
    /// Note id (primary key).
    pub id: String,
    /// Note body, stored as the document's JSON payload.
    pub body: serde_json::Value,
}

/// Note state after the upsert.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NoteReply {
    /// Stored payload size in bytes, read back from the store.
    pub size: usize,
}

pub(crate) async fn note(Json(input): Json<NoteRequest>) -> Result<Json<NoteReply>> {
    let notes = docstore::open(&input.store).await?;

    notes.upsert(&input.id, &input.body).await?;

    let stored = notes.get(input.id).await?.context("note missing after upsert")?;
    Ok(Json(NoteReply {
        size: stored.data.len(),
    }))
}
