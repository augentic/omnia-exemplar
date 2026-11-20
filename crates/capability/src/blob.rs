//! Blobstore example: archive a payload and report its stored size.

use anyhow::Context as _;
use axum::Json;
use axum::routing::post;
use omnia_sdk::blobstore::{self, admin, store};
use omnia_sdk::{Handlers, Result};
use serde::{Deserialize, Serialize};

pub(crate) const ARCHIVE: &str = "/examples/archive";

pub(crate) fn handlers() -> Handlers {
    Handlers::new().route(ARCHIVE, post(archive))
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

pub(crate) async fn archive(Json(input): Json<ArchiveRequest>) -> Result<Json<ArchiveReply>> {
    let container = if store::container_exists(input.container.clone()).await? {
        blobstore::open(&input.container).await?
    } else {
        admin::create_container(input.container).await?
    };
    container.write(&input.name, input.payload.as_bytes()).await?;

    let info = container.stat(input.name).await?.context("object missing after archive")?;
    Ok(Json(ArchiveReply { size: info.size }))
}
