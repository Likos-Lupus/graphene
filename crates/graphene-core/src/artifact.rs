use crate::{ArtifactId, Sha1Digest, Sha256Digest, Sha512Digest};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Provider-neutral classification for an acquired artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ArtifactKind {
    /// Generic opaque content.
    Generic,
    /// Metadata content that is still provider-neutral at this boundary.
    Metadata,
    /// Executable or binary-like content.
    Binary,
}

/// Cache behavior for an artifact acquisition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum CachePolicy {
    /// Reuse an existing object only after validating its declared integrity.
    #[default]
    UseVerified,
    /// Re-download, verify, and safely replace the cache object.
    Refresh,
}

/// An ordered, generic artifact source.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSource {
    url: String,
    priority: u32,
    label: Option<String>,
}

impl ArtifactSource {
    /// Creates a source with default priority `0`.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            priority: 0,
            label: None,
        }
    }

    /// Sets deterministic source priority. Lower values are tried first.
    #[must_use]
    pub const fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Sets a non-sensitive mirror/source label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Returns the source URL for transport use.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Returns deterministic source priority.
    #[must_use]
    pub const fn priority(&self) -> u32 {
        self.priority
    }

    /// Returns the optional safe source label.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
}

impl fmt::Debug for ArtifactSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArtifactSource")
            .field("url", &"<redacted-url>")
            .field("priority", &self.priority)
            .field("label", &self.label)
            .finish()
    }
}

/// Expected integrity values for an artifact.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactIntegrity {
    sha1: Option<Sha1Digest>,
    sha256: Option<Sha256Digest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sha512: Option<Sha512Digest>,
}

impl ArtifactIntegrity {
    /// Creates an empty integrity declaration.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            sha1: None,
            sha256: None,
            sha512: None,
        }
    }

    /// Sets an expected SHA-1 digest.
    #[must_use]
    pub const fn with_sha1(mut self, digest: Sha1Digest) -> Self {
        self.sha1 = Some(digest);
        self
    }

    /// Sets an expected SHA-256 digest.
    #[must_use]
    pub const fn with_sha256(mut self, digest: Sha256Digest) -> Self {
        self.sha256 = Some(digest);
        self
    }

    /// Sets an expected SHA-512 digest.
    #[must_use]
    pub const fn with_sha512(mut self, digest: Sha512Digest) -> Self {
        self.sha512 = Some(digest);
        self
    }

    /// Returns the expected SHA-1 digest.
    #[must_use]
    pub const fn sha1(&self) -> Option<Sha1Digest> {
        self.sha1
    }

    /// Returns the expected SHA-256 digest.
    #[must_use]
    pub const fn sha256(&self) -> Option<Sha256Digest> {
        self.sha256
    }

    /// Returns the expected SHA-512 digest.
    #[must_use]
    pub const fn sha512(&self) -> Option<Sha512Digest> {
        self.sha512
    }

    /// Returns whether at least one trustworthy digest is declared.
    #[must_use]
    pub const fn is_verifiable(&self) -> bool {
        self.sha1.is_some() || self.sha256.is_some() || self.sha512.is_some()
    }
}

/// Generic artifact declaration shared by future providers and installers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// Strong artifact identity used for operation correlation.
    pub id: ArtifactId,
    /// Provider-neutral artifact classification.
    pub kind: ArtifactKind,
    /// Ordered candidate sources.
    pub sources: Vec<ArtifactSource>,
    /// Expected integrity values.
    pub integrity: ArtifactIntegrity,
    /// Expected byte size when known.
    pub expected_size: Option<u64>,
    /// Cache lookup behavior.
    pub cache_policy: CachePolicy,
}

impl Artifact {
    /// Creates a generic artifact declaration.
    #[must_use]
    pub fn new(sources: Vec<ArtifactSource>, integrity: ArtifactIntegrity) -> Self {
        Self {
            id: ArtifactId::new(),
            kind: ArtifactKind::Generic,
            sources,
            integrity,
            expected_size: None,
            cache_policy: CachePolicy::UseVerified,
        }
    }

    /// Sets an expected byte size.
    #[must_use]
    pub const fn with_expected_size(mut self, size: u64) -> Self {
        self.expected_size = Some(size);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrity_reports_verifiability() {
        assert!(!ArtifactIntegrity::none().is_verifiable());
        let digest = "a9993e364706816aba3e25717850c26c9cd0d89d"
            .parse()
            .expect("valid sha1");
        assert!(ArtifactIntegrity::none().with_sha1(digest).is_verifiable());

        let sha512: Sha512Digest = "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
                                    2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
            .parse()
            .expect("valid sha512");
        assert!(
            ArtifactIntegrity::none()
                .with_sha512(sha512)
                .is_verifiable()
        );
    }

    #[test]
    fn integrity_deserializes_legacy_payload_without_sha512() {
        let legacy = r#"{"sha1":"a9993e364706816aba3e25717850c26c9cd0d89d"}"#;
        let integrity: ArtifactIntegrity = serde_json::from_str(legacy).expect("legacy payload");
        assert_eq!(
            integrity.sha1(),
            Some(
                "a9993e364706816aba3e25717850c26c9cd0d89d"
                    .parse()
                    .expect("sha1")
            )
        );
        assert_eq!(integrity.sha512(), None);

        let round_trip = serde_json::to_string(&integrity).expect("serialize");
        assert!(!round_trip.contains("sha512"));
    }

    #[test]
    fn artifact_source_debug_redacts_url() {
        let source = ArtifactSource::new("https://user:secret@example.invalid/file?token=secret")
            .with_label("mirror");
        let debug = format!("{source:?}");
        assert!(!debug.contains("secret"));
        assert!(!debug.contains("token"));
        assert!(debug.contains("mirror"));
    }
}
