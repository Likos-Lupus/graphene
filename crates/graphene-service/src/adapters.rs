use crate::{ArtifactService, DownloadDisposition, context::ServiceContext};
use graphene_core::{Artifact, ErrorCode, ErrorKind, GrapheneError, OperationHandle, Result};
use graphene_install::{AcquiredArtifact, AcquisitionDisposition, ArtifactAcquirer};
use graphene_providers::MetadataArtifactAcquirer;
use std::{future::Future, pin::Pin, sync::Arc};

#[derive(Clone)]
pub(crate) struct ServiceArtifactAcquirer {
    artifacts: ArtifactService,
}

impl ServiceArtifactAcquirer {
    pub(crate) fn new(context: Arc<ServiceContext>) -> Self {
        Self {
            artifacts: ArtifactService::new(context),
        }
    }
}

impl ArtifactAcquirer for ServiceArtifactAcquirer {
    fn acquire<'a>(
        &'a self,
        artifact: Artifact,
        parent: OperationHandle,
    ) -> Pin<Box<dyn Future<Output = Result<AcquiredArtifact>> + Send + 'a>> {
        Box::pin(async move {
            let verified = self
                .artifacts
                .acquire(artifact, Some(&parent))
                .await_result()
                .await?;
            Ok(AcquiredArtifact {
                artifact_id: verified.artifact_id,
                path: verified.path,
                bytes: verified.bytes,
                sha1: verified.sha1,
                sha256: verified.sha256,
                disposition: match verified.disposition {
                    DownloadDisposition::CacheHit => AcquisitionDisposition::CacheHit,
                    DownloadDisposition::Downloaded => AcquisitionDisposition::Downloaded,
                },
            })
        })
    }
}

#[derive(Clone)]
pub(crate) struct ServiceMetadataAcquirer {
    artifacts: ArtifactService,
}

impl ServiceMetadataAcquirer {
    pub(crate) fn new(context: Arc<ServiceContext>) -> Self {
        Self {
            artifacts: ArtifactService::new(context),
        }
    }
}

impl MetadataArtifactAcquirer for ServiceMetadataAcquirer {
    fn acquire<'a>(
        &'a self,
        artifact: Artifact,
        parent: OperationHandle,
        max_bytes: usize,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
        Box::pin(async move {
            let verified = self
                .artifacts
                .acquire(artifact, Some(&parent))
                .await_result()
                .await?;
            if verified.bytes > max_bytes as u64 {
                return Err(GrapheneError::new(
                    ErrorCode::MinecraftMetadataInvalid,
                    ErrorKind::Minecraft,
                    "verified metadata artifact exceeds its parse bound",
                )
                .with_context("max_bytes", max_bytes.to_string()));
            }

            let bytes = tokio::fs::read(&verified.path).await.map_err(|source| {
                GrapheneError::new(
                    ErrorCode::MinecraftMetadataInvalid,
                    ErrorKind::Minecraft,
                    "failed to read verified metadata artifact",
                )
                .with_source(source)
            })?;
            if bytes.len() as u64 != verified.bytes {
                return Err(GrapheneError::new(
                    ErrorCode::MinecraftMetadataInvalid,
                    ErrorKind::Minecraft,
                    "verified metadata artifact changed before parsing",
                ));
            }

            Ok(bytes)
        })
    }
}
