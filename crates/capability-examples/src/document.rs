//! Document-store example: upsert a JSON note and read it back.

use anyhow::Context as _;
use omnia_guest::api::{CallContext, Operation, Provider};
use omnia_guest::document_store::Document;
use omnia_guest::{DocumentStore, Error, Result};
use serde::{Deserialize, Serialize};

/// Upsert a JSON note document.
#[derive(Debug, Clone, Deserialize)]
pub struct NoteRequest {
    /// Document collection.
    pub store: String,
    /// Note id (primary key).
    pub id: String,
    /// Note body, stored as the document's JSON payload.
    pub body: serde_json::Value,
}

/// Note state after the upsert.
#[derive(Debug, Clone, Serialize)]
pub struct NoteReply {
    /// Stored payload size in bytes, read back from the store.
    pub size: usize,
}

impl<P> Operation<P> for NoteRequest
where
    P: Provider + DocumentStore,
{
    type Error = Error;
    type Input = Self;
    type Output = NoteReply;

    async fn call(input: Self, context: CallContext<'_, P>) -> Result<NoteReply> {
        let provider = context.provider;

        let document = Document {
            id: input.id.clone(),
            data: serde_json::to_vec(&input.body).context("failed to serialize note body")?,
        };
        DocumentStore::put(provider, &input.store, &document).await?;

        let stored = DocumentStore::get(provider, &input.store, &input.id)
            .await?
            .context("note missing after upsert")?;
        Ok(NoteReply {
            size: stored.data.len(),
        })
    }
}
