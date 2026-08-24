use crate::{
    id::ContentFileRef,
    model::{file::ContentFile, version::ContentVersion},
};
use graphene_core::{Sha1Digest, Sha256Digest};
use serde::{Deserialize, Serialize};

/// Request item for looking up a local file by exact digests/fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileMatchItem {
    pub sha1: Option<Sha1Digest>,
    pub sha256: Option<Sha256Digest>,
    pub murmur2_fingerprint: Option<u32>,
    pub size: u64,
}

impl FileMatchItem {
    #[must_use]
    pub const fn new(
        sha1: Option<Sha1Digest>,
        sha256: Option<Sha256Digest>,
        murmur2_fingerprint: Option<u32>,
        size: u64,
    ) -> Self {
        Self {
            sha1,
            sha256,
            murmur2_fingerprint,
            size,
        }
    }
}

/// Batch request for exact file recognition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMatchRequest {
    pub items: Vec<FileMatchItem>,
}

/// Status of an exact match lookup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExactMatchStatus {
    /// Exact unique match found.
    Matched {
        version: Box<ContentVersion>,
        file: Box<ContentFile>,
    },
    /// Multiple files matched the identical bytes.
    Ambiguous(Vec<ContentFileRef>),
    /// No matching file known to the provider.
    Unmatched,
}

/// Result of matching a single local file query item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentFileMatch {
    pub item: FileMatchItem,
    pub status: ExactMatchStatus,
}
