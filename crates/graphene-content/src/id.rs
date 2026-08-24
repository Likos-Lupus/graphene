use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const MAX_PROVIDER_ID_LEN: usize = 64;
pub const MAX_OPAQUE_ID_LEN: usize = 128;
pub const MAX_ENTRY_ID_LEN: usize = 64;

/// Stable identifier for a content provider (e.g. "modrinth", "curseforge").
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentProviderId(String);

impl ContentProviderId {
    /// Creates and validates a content provider identifier.
    pub fn new(id: impl Into<String>) -> Result<Self> {
        let s = id.into();
        Self::validate_str(&s)?;
        Ok(Self(s))
    }

    /// Predefined Modrinth provider identifier.
    #[must_use]
    pub const fn modrinth() -> Self {
        Self(String::new()) // Helper replaced below
    }

    fn validate_str(s: &str) -> Result<()> {
        if s.is_empty() || s.len() > MAX_PROVIDER_ID_LEN {
            return Err(GrapheneError::new(
                ErrorCode::ConfigInvalid,
                ErrorKind::Configuration,
                format!("content provider id length must be between 1 and {MAX_PROVIDER_ID_LEN}"),
            ));
        }
        if !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(GrapheneError::new(
                ErrorCode::ConfigInvalid,
                ErrorKind::Configuration,
                "content provider id contains invalid characters (allowed: alphanumeric, -, _)",
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ContentProviderId {
    pub const MODRINTH: &'static str = "modrinth";
    pub const CURSEFORGE: &'static str = "curseforge";
}

impl fmt::Debug for ContentProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ContentProviderId").field(&self.0).finish()
    }
}

impl fmt::Display for ContentProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Provider-scoped opaque reference to a remote content project.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ContentProjectRef {
    pub provider: ContentProviderId,
    pub project_id: String,
}

impl ContentProjectRef {
    pub fn new(provider: ContentProviderId, project_id: impl Into<String>) -> Result<Self> {
        let project_id = project_id.into();
        validate_opaque_id("project_id", &project_id)?;
        Ok(Self {
            provider,
            project_id,
        })
    }
}

impl fmt::Debug for ContentProjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.provider, self.project_id)
    }
}

impl fmt::Display for ContentProjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.provider, self.project_id)
    }
}

/// Provider-scoped opaque reference to a specific version of a project.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ContentVersionRef {
    pub provider: ContentProviderId,
    pub project_id: String,
    pub version_id: String,
}

impl ContentVersionRef {
    pub fn new(
        provider: ContentProviderId,
        project_id: impl Into<String>,
        version_id: impl Into<String>,
    ) -> Result<Self> {
        let project_id = project_id.into();
        let version_id = version_id.into();
        validate_opaque_id("project_id", &project_id)?;
        validate_opaque_id("version_id", &version_id)?;
        Ok(Self {
            provider,
            project_id,
            version_id,
        })
    }

    #[must_use]
    pub fn project_ref(&self) -> ContentProjectRef {
        ContentProjectRef {
            provider: self.provider.clone(),
            project_id: self.project_id.clone(),
        }
    }
}

impl fmt::Debug for ContentVersionRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.provider, self.project_id, self.version_id
        )
    }
}

impl fmt::Display for ContentVersionRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.provider, self.project_id, self.version_id
        )
    }
}

/// Provider-scoped opaque reference to an exact downloadable file within a version.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ContentFileRef {
    pub provider: ContentProviderId,
    pub project_id: String,
    pub version_id: String,
    pub file_id: String,
}

impl ContentFileRef {
    pub fn new(
        provider: ContentProviderId,
        project_id: impl Into<String>,
        version_id: impl Into<String>,
        file_id: impl Into<String>,
    ) -> Result<Self> {
        let project_id = project_id.into();
        let version_id = version_id.into();
        let file_id = file_id.into();
        validate_opaque_id("project_id", &project_id)?;
        validate_opaque_id("version_id", &version_id)?;
        validate_opaque_id("file_id", &file_id)?;
        Ok(Self {
            provider,
            project_id,
            version_id,
            file_id,
        })
    }

    #[must_use]
    pub fn version_ref(&self) -> ContentVersionRef {
        ContentVersionRef {
            provider: self.provider.clone(),
            project_id: self.project_id.clone(),
            version_id: self.version_id.clone(),
        }
    }

    #[must_use]
    pub fn project_ref(&self) -> ContentProjectRef {
        ContentProjectRef {
            provider: self.provider.clone(),
            project_id: self.project_id.clone(),
        }
    }
}

impl fmt::Debug for ContentFileRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}",
            self.provider, self.project_id, self.version_id, self.file_id
        )
    }
}

impl fmt::Display for ContentFileRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}",
            self.provider, self.project_id, self.version_id, self.file_id
        )
    }
}

/// Stable Graphene-local managed entry identity.
/// Survives enable/disable and version updates.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentEntryId(String);

impl ContentEntryId {
    /// Generates a new unique random entry ID.
    #[must_use]
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Creates and validates a content entry ID.
    pub fn new(id: impl Into<String>) -> Result<Self> {
        let s = id.into();
        if s.is_empty() || s.len() > MAX_ENTRY_ID_LEN {
            return Err(GrapheneError::new(
                ErrorCode::ConfigInvalid,
                ErrorKind::Configuration,
                format!("content entry id length must be between 1 and {MAX_ENTRY_ID_LEN}"),
            ));
        }
        if !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(GrapheneError::new(
                ErrorCode::ConfigInvalid,
                ErrorKind::Configuration,
                "content entry id contains invalid characters",
            ));
        }
        Ok(Self(s))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ContentEntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ContentEntryId").field(&self.0).finish()
    }
}

impl fmt::Display for ContentEntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn validate_opaque_id(field: &'static str, s: &str) -> Result<()> {
    if s.is_empty() || s.len() > MAX_OPAQUE_ID_LEN {
        return Err(GrapheneError::new(
            ErrorCode::ConfigInvalid,
            ErrorKind::Configuration,
            format!("{field} length must be between 1 and {MAX_OPAQUE_ID_LEN}"),
        ));
    }
    if s.chars().any(|c| c.is_control() || c == '/' || c == '\\') {
        return Err(GrapheneError::new(
            ErrorCode::ConfigInvalid,
            ErrorKind::Configuration,
            format!("{field} contains invalid control or path characters"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_id_validation() {
        assert!(ContentProviderId::new("modrinth").is_ok());
        assert!(ContentProviderId::new("curseforge").is_ok());
        assert!(ContentProviderId::new("").is_err());
        assert!(ContentProviderId::new("has space").is_err());
        assert!(ContentProviderId::new("has/slash").is_err());
    }

    #[test]
    fn refs_round_trip() {
        let provider = ContentProviderId::new("modrinth").unwrap();
        let proj = ContentProjectRef::new(provider.clone(), "sodium").unwrap();
        let ver = ContentVersionRef::new(provider.clone(), "sodium", "v0.5.8").unwrap();
        let file = ContentFileRef::new(provider, "sodium", "v0.5.8", "sodium-0.5.8.jar").unwrap();

        assert_eq!(ver.project_ref(), proj);
        assert_eq!(file.version_ref(), ver);
        assert_eq!(file.project_ref(), proj);
        assert_eq!(file.to_string(), "modrinth:sodium:v0.5.8:sodium-0.5.8.jar");
    }

    #[test]
    fn entry_id_generation_and_validation() {
        let entry = ContentEntryId::generate();
        assert!(!entry.as_str().is_empty());
        assert!(ContentEntryId::new(entry.as_str()).is_ok());
        assert!(ContentEntryId::new("").is_err());
    }
}
