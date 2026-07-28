//! Blobstore example: archive a payload and report its stored size.

use omnia_guest::api::{CallContext, Operation, Provider};
use omnia_guest::{BlobStore, Error, Result};
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

impl<P> Operation<P> for ArchiveRequest
where
    P: Provider + BlobStore,
{
    type Error = Error;
    type Input = Self;
    type Output = ArchiveReply;

    async fn call(input: Self, context: CallContext<'_, P>) -> Result<ArchiveReply> {
        let provider = context.provider;

        if !BlobStore::container_exists(provider, &input.container).await? {
            BlobStore::create_container(provider, &input.container).await?;
        }
        BlobStore::put(provider, &input.container, &input.name, input.payload.as_bytes()).await?;

        let info = BlobStore::object_info(provider, &input.container, &input.name).await?;
        Ok(ArchiveReply { size: info.size })
    }
}
