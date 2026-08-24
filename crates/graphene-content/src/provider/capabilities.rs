use serde::{Deserialize, Serialize};

/// Feature capabilities supported by a specific content provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct ContentProviderCapabilities {
    pub search: bool,
    pub project_lookup: bool,
    pub version_listing: bool,
    pub exact_version_lookup: bool,
    pub exact_file_matching: bool,
    pub batch_file_matching: bool,
    pub update_lookup: bool,
    pub anonymous_read: bool,
    pub requires_credentials: bool,
}

impl ContentProviderCapabilities {
    /// Capabilities for Modrinth (anonymous read, sha1 matching, search, updates).
    #[must_use]
    pub const fn modrinth() -> Self {
        Self {
            search: true,
            project_lookup: true,
            version_listing: true,
            exact_version_lookup: true,
            exact_file_matching: true,
            batch_file_matching: true,
            update_lookup: true,
            anonymous_read: true,
            requires_credentials: false,
        }
    }

    /// Capabilities for CurseForge (requires API key, murmur2 fingerprint matching, search).
    #[must_use]
    pub const fn curseforge() -> Self {
        Self {
            search: true,
            project_lookup: true,
            version_listing: true,
            exact_version_lookup: true,
            exact_file_matching: true,
            batch_file_matching: true,
            update_lookup: true,
            anonymous_read: false,
            requires_credentials: true,
        }
    }
}
