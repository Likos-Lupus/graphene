use graphene_core::{ArtifactIntegrity, ErrorCode, ErrorKind, GrapheneError, Result};
use std::path::{Path, PathBuf};

/// Digest algorithm used for deterministic cache identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheAlgorithm {
    Sha1,
    Sha256,
}

/// Deterministic committed cache address.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheAddress {
    algorithm: CacheAlgorithm,
    digest: String,
    path: PathBuf,
}

impl CacheAddress {
    pub(crate) fn from_integrity(root: &Path, integrity: &ArtifactIntegrity) -> Result<Self> {
        let (algorithm, algorithm_dir, digest) = if let Some(digest) = integrity.sha256() {
            (CacheAlgorithm::Sha256, "sha256", digest.to_string())
        } else if let Some(digest) = integrity.sha1() {
            (CacheAlgorithm::Sha1, "sha1", digest.to_string())
        } else {
            return Err(GrapheneError::new(
                ErrorCode::CacheIdentityUnavailable,
                ErrorKind::Integrity,
                "cache identity requires SHA-1 or SHA-256 integrity",
            ));
        };

        let shard = &digest[..2];
        Ok(Self {
            algorithm,
            path: root
                .join("cache")
                .join("objects")
                .join(algorithm_dir)
                .join(shard)
                .join(&digest),
            digest,
        })
    }

    /// Returns the algorithm selected for identity (SHA-256 preferred over SHA-1).
    #[must_use]
    pub const fn algorithm(&self) -> CacheAlgorithm {
        self.algorithm
    }

    /// Returns the normalized digest string.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Returns the committed object path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}
