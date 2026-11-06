//! Blobstore example: archive a payload and report its stored size.

use axum::Json;
use axum::routing::post;
use common::Capabilities;
use omnia_sdk::api::Context;
use omnia_sdk::api::http::Routes;
use omnia_sdk::{BlobStore, Result};
use serde::{Deserialize, Serialize};

pub(crate) const ARCHIVE: &str = "/examples/archive";

pub(crate) fn routes<P: Capabilities>() -> Routes<P> {
    Routes::new().route(ARCHIVE, post(archive))
}

/// Archive a payload into a blobstore container.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArchiveRequest {
    /// Container receiving the object.
    pub container: String,
    /// Object name within the container.
    pub name: String,
    /// Payload stored as the object body.
    pub payload: String,
}

/// Metadata reported after archiving.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArchiveReply {
    /// Stored object size in bytes.
    pub size: u64,
}

pub(crate) async fn archive<P>(
    context: Context<P>, Json(input): Json<ArchiveRequest>,
) -> Result<Json<ArchiveReply>>
where
    P: BlobStore + 'static,
{
    let provider = context.provider();

    if !BlobStore::container_exists(provider, &input.container).await? {
        BlobStore::create_container(provider, &input.container).await?;
    }
    BlobStore::put(provider, &input.container, &input.name, input.payload.as_bytes()).await?;

    let info = BlobStore::object_info(provider, &input.container, &input.name).await?;
    Ok(Json(ArchiveReply { size: info.size }))
}
