use graphene_core::Sha256Digest;
use serde::{Deserialize, Serialize};

/// Immutable identity of the bytes a pack inspection observed.
///
/// The snapshot intentionally carries no URL and no local path: planning pins this digest, and
/// execution resolves it through the content-addressed cache only. If the cache object is absent,
/// execution fails with a typed stale/unavailable error instead of refetching mutable bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackSourceSnapshot {
    source_sha256: Sha256Digest,
    size: u64,
}

impl PackSourceSnapshot {
    #[must_use]
    pub const fn new(source_sha256: Sha256Digest, size: u64) -> Self {
        Self {
            source_sha256,
            size,
        }
    }

    #[must_use]
    pub const fn sha256(&self) -> &Sha256Digest {
        &self.source_sha256
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }
}
