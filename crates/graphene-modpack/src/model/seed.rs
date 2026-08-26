use crate::archive::PackPath;
use graphene_core::Sha256Digest;
use serde::{Deserialize, Serialize};

/// Deterministic override layer ordering. Lower values apply first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SeedLayer(u8);

impl SeedLayer {
    /// Base override layer declared by the format.
    #[must_use]
    pub const fn base() -> Self {
        Self(0)
    }

    /// Later client-override layer with documented precedence over [`SeedLayer::base`].
    #[must_use]
    pub const fn client_override() -> Self {
        Self(1)
    }

    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

/// One initial user/game payload entry extracted from the pack archive.
///
/// Seed entries are hashed during inspection so plans are deterministic; they are applied
/// transactionally during creation but are not immutable repair authority afterward.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedSeedEntry {
    archive_entry: String,
    destination: PackPath,
    layer: SeedLayer,
    size: u64,
    sha256: Sha256Digest,
}

impl NormalizedSeedEntry {
    #[must_use]
    pub const fn new(
        archive_entry: String,
        destination: PackPath,
        layer: SeedLayer,
        size: u64,
        sha256: Sha256Digest,
    ) -> Self {
        Self {
            archive_entry,
            destination,
            layer,
            size,
            sha256,
        }
    }

    #[must_use]
    pub fn archive_entry(&self) -> &str {
        &self.archive_entry
    }

    #[must_use]
    pub fn destination(&self) -> &PackPath {
        &self.destination
    }

    #[must_use]
    pub const fn layer(&self) -> SeedLayer {
        self.layer
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn sha256(&self) -> &Sha256Digest {
        &self.sha256
    }
}
