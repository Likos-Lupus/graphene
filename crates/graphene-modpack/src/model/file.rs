use crate::archive::{PackPath, limits::MAX_DISPLAY_STRING_CHARS};
use crate::error::PackError;
use graphene_core::{ArtifactIntegrity, Sha256Digest};
use serde::{Deserialize, Serialize};

/// Client-side selection semantics for one managed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSelection {
    /// Installed on every client import.
    Required,
    /// Installed only when the host selects the referenced choice.
    Optional { choice_id: String },
    /// Declared unsupported for the client side and excluded from client plans.
    ExcludedForClient,
}

/// A safe, persistable remote source for a managed file.
///
/// Construction applies the persisted-URL policy (HTTPS-only scheme, allowed host, no userinfo,
/// bounded length, no fragment, no known secret-bearing query keys). URLs that cannot satisfy the
/// policy are never stored; such artifacts become cache-only after verified acquisition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedSource(String);

impl ManagedSource {
    /// Validates one candidate source URL against the persistable-source policy.
    ///
    /// `allowed_host` is the policy predicate supplied by the format adapter or service policy.
    pub fn parse(raw: &str, allowed_host: impl Fn(&str) -> bool) -> Result<Self, PackError> {
        if raw.len() > 2_048 {
            return Err(PackError::new(
                graphene_core::ErrorCode::PackArtifactSourceUnsafe,
                "managed source URL exceeds its length bound",
            ));
        }
        let url = url::Url::parse(raw).map_err(|_| {
            PackError::new(
                graphene_core::ErrorCode::PackArtifactSourceUnsafe,
                "managed source URL is not a valid URL",
            )
        })?;
        if url.scheme() != "https" {
            return Err(PackError::new(
                graphene_core::ErrorCode::PackArtifactSourceUnsafe,
                "managed source must use HTTPS",
            ));
        }
        let host = url.host_str().unwrap_or_default();
        if host.is_empty() || !allowed_host(host) {
            return Err(PackError::new(
                graphene_core::ErrorCode::PackArtifactSourceUnsafe,
                "managed source host is not allowed by pack policy",
            ));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(PackError::new(
                graphene_core::ErrorCode::PackArtifactSourceUnsafe,
                "managed source must not carry userinfo",
            ));
        }
        if url.fragment().is_some() {
            return Err(PackError::new(
                graphene_core::ErrorCode::PackArtifactSourceUnsafe,
                "managed source must not carry a fragment",
            ));
        }
        if query_carries_secrets(url.query()) {
            return Err(PackError::new(
                graphene_core::ErrorCode::PackArtifactSourceUnsafe,
                "managed source query carries a forbidden credential key",
            ));
        }
        Ok(Self(raw.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

const FORBIDDEN_QUERY_KEYS: [&str; 12] = [
    "token",
    "access_token",
    "api_key",
    "apikey",
    "key",
    "secret",
    "signature",
    "sig",
    "x-amz-signature",
    "x-amz-credential",
    "x-goog-signature",
    "expires",
];

fn query_carries_secrets(query: Option<&str>) -> bool {
    let Some(query) = query else {
        return false;
    };
    query.split('&').any(|pair| {
        let key = pair.split('=').next().unwrap_or_default();
        let normalized = key.to_ascii_lowercase();
        FORBIDDEN_QUERY_KEYS
            .iter()
            .any(|forbidden| *forbidden == normalized)
    })
}

/// Exact provider catalog provenance for a managed file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ProviderFileRef {
    CurseForge { project_id: String, file_id: String },
}

impl ProviderFileRef {
    /// Stable display identity used in lockfile content entries.
    #[must_use]
    pub fn project_id(&self) -> &str {
        match self {
            Self::CurseForge { project_id, .. } => project_id,
        }
    }

    #[must_use]
    pub fn file_id(&self) -> &str {
        match self {
            Self::CurseForge { file_id, .. } => file_id,
        }
    }
}

/// Exact provider file declaration awaiting service-owned catalog resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingProviderFile {
    provider_ref: ProviderFileRef,
    selection: FileSelection,
}

impl PendingProviderFile {
    pub fn new(provider_ref: ProviderFileRef, selection: FileSelection) -> Result<Self, PackError> {
        validate_selection(&selection)?;
        Ok(Self {
            provider_ref,
            selection,
        })
    }

    #[must_use]
    pub const fn provider_ref(&self) -> &ProviderFileRef {
        &self.provider_ref
    }

    #[must_use]
    pub const fn selection(&self) -> &FileSelection {
        &self.selection
    }

    #[must_use]
    pub fn stable_id(&self) -> String {
        match &self.provider_ref {
            ProviderFileRef::CurseForge {
                project_id,
                file_id,
            } => format!("curseforge:{project_id}:{file_id}"),
        }
    }
}

/// Non-executable hint about what kind of payload a managed file represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ContentHint {
    Mod,
    ResourcePack,
    ShaderPack,
}

/// Immutable file embedded in the pinned pack archive and promoted out of mutable seed state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedPackFile {
    archive_entry: String,
    destination: PackPath,
    size: u64,
    sha256: Sha256Digest,
    content_hint: Option<ContentHint>,
}

impl EmbeddedPackFile {
    #[must_use]
    pub const fn new(
        archive_entry: String,
        destination: PackPath,
        size: u64,
        sha256: Sha256Digest,
        content_hint: Option<ContentHint>,
    ) -> Self {
        Self {
            archive_entry,
            destination,
            size,
            sha256,
            content_hint,
        }
    }

    #[must_use]
    pub fn archive_entry(&self) -> &str {
        &self.archive_entry
    }

    #[must_use]
    pub const fn destination(&self) -> &PackPath {
        &self.destination
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn sha256(&self) -> &Sha256Digest {
        &self.sha256
    }

    #[must_use]
    pub const fn content_hint(&self) -> Option<ContentHint> {
        self.content_hint
    }
}

/// One immutable desired-state file declared by a pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedPackFile {
    destination: PackPath,
    selection: FileSelection,
    size: u64,
    integrity: ArtifactIntegrity,
    sources: Vec<ManagedSource>,
    provider_ref: Option<ProviderFileRef>,
    content_hint: Option<ContentHint>,
}

impl NormalizedPackFile {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        destination: PackPath,
        selection: FileSelection,
        size: u64,
        integrity: ArtifactIntegrity,
        sources: Vec<ManagedSource>,
        provider_ref: Option<ProviderFileRef>,
        content_hint: Option<ContentHint>,
    ) -> Result<Self, PackError> {
        if !integrity.is_verifiable() {
            return Err(PackError::new(
                graphene_core::ErrorCode::PackArtifactUnverifiable,
                "managed file has no trustworthy integrity declaration",
            ));
        }
        if sources.is_empty() && provider_ref.is_none() {
            // Remote-declared files must have at least one safe source or an exact provider
            // reference that resolution will turn into sources. Purely local embedded files are
            // modeled as seed promotions instead of this type.
            return Err(PackError::new(
                graphene_core::ErrorCode::PackArtifactUnverifiable,
                "remote managed file has neither a safe source nor provider identity",
            ));
        }
        validate_selection(&selection)?;
        Ok(Self {
            destination,
            selection,
            size,
            integrity,
            sources,
            provider_ref,
            content_hint,
        })
    }

    #[must_use]
    pub fn destination(&self) -> &PackPath {
        &self.destination
    }

    #[must_use]
    pub const fn selection(&self) -> &FileSelection {
        &self.selection
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn integrity(&self) -> &ArtifactIntegrity {
        &self.integrity
    }

    #[must_use]
    pub fn sources(&self) -> &[ManagedSource] {
        &self.sources
    }

    #[must_use]
    pub const fn provider_ref(&self) -> Option<&ProviderFileRef> {
        self.provider_ref.as_ref()
    }

    #[must_use]
    pub const fn content_hint(&self) -> Option<ContentHint> {
        self.content_hint
    }
}

fn validate_selection(selection: &FileSelection) -> Result<(), PackError> {
    if let FileSelection::Optional { choice_id } = selection
        && (choice_id.is_empty() || choice_id.chars().count() > MAX_DISPLAY_STRING_CHARS)
    {
        return Err(PackError::manifest("optional choice id exceeds its bound"));
    }
    Ok(())
}
