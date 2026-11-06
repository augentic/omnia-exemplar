//! Document-store example: upsert a JSON note and read it back.

use anyhow::Context as _;
use axum::Json;
use axum::routing::post;
use common::Capabilities;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::docstore::Document;
use omnia_sdk::{DocumentStore, Result};
use serde::{Deserialize, Serialize};

pub(crate) const NOTE: &str = "/examples/note";

pub(crate) fn handlers<P: Capabilities>() -> Handlers<P> {
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

pub(crate) async fn note<P>(
    context: Context<P>, Json(input): Json<NoteRequest>,
) -> Result<Json<NoteReply>>
where
    P: DocumentStore + 'static,
{
    let provider = context.provider();

    let document = Document {
        id: input.id.clone(),
        data: serde_json::to_vec(&input.body).context("failed to serialize note body")?,
    };
    DocumentStore::put(provider, &input.store, &document).await?;

    let stored = DocumentStore::get(provider, &input.store, &input.id)
        .await?
        .context("note missing after upsert")?;
    Ok(Json(NoteReply {
        size: stored.data.len(),
    }))
}
