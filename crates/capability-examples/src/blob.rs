//! Blobstore example: archive a payload and report its stored size.

use omnia_guest::api::{CallContext, Provider};
use omnia_guest::{BlobStore, Result};
use serde::{Deserialize, Serialize};

/// Archive a payload into a blobstore container.
#[derive(Debug, Clone, Deserialize)]
pub struct ArchiveRequest {
    /// Container receiving the object.
    pub container: String,
    /// Object name within the container.
    pub name: String,
    /// Payload stored as the object body.
    pub payload: String,
}

/// Metadata reported after archiving.
#[derive(Debug, Clone, Serialize)]
pub struct ArchiveReply {
    /// Stored object size in bytes.
    pub size: u64,
}

#[omnia_guest::operation]
async fn archive_request<P>(
    input: ArchiveRequest, context: CallContext<'_, P>,
) -> Result<ArchiveReply>
where
    P: Provider + BlobStore,
{
    let provider = context.provider;

    if !BlobStore::container_exists(provider, &input.container).await? {
        BlobStore::create_container(provider, &input.container).await?;
    }
    BlobStore::put(provider, &input.container, &input.name, input.payload.as_bytes()).await?;

    let info = BlobStore::object_info(provider, &input.container, &input.name).await?;
    Ok(ArchiveReply { size: info.size })
}
