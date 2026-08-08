use graphene_core::{Artifact, ArtifactId, OperationHandle, Result, Sha1Digest, Sha256Digest};
use std::{future::Future, path::PathBuf, pin::Pin};

/// Result source without leaking service-layer types across the dependency inversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AcquisitionDisposition {
    CacheHit,
    Downloaded,
}

/// Verified acquisition result returned to installation through the provider-neutral port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquiredArtifact {
    pub artifact_id: ArtifactId,
    pub path: PathBuf,
    pub bytes: u64,
    pub sha1: Sha1Digest,
    pub sha256: Sha256Digest,
    pub disposition: AcquisitionDisposition,
}

/// Stable installer-owned port implemented by `graphene-service` using Phase 0 acquisition.
pub trait ArtifactAcquirer: Send + Sync {
    fn acquire<'a>(
        &'a self,
        artifact: Artifact,
        parent: OperationHandle,
    ) -> Pin<Box<dyn Future<Output = Result<AcquiredArtifact>> + Send + 'a>>;
}
