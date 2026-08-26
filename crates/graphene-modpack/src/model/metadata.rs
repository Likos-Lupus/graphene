use crate::archive::limits::MAX_DISPLAY_STRING_CHARS;
use crate::error::PackError;
use serde::{Deserialize, Serialize};

/// Bounded, non-authoritative display metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackMetadata {
    name: String,
    version: Option<String>,
    summary: Option<String>,
    authors: Vec<String>,
}

impl PackMetadata {
    pub fn new(
        name: impl Into<String>,
        version: Option<String>,
        summary: Option<String>,
        authors: Vec<String>,
    ) -> Result<Self, PackError> {
        let name = name.into();
        validate_bounded("name", &name)?;
        if let Some(version) = &version {
            validate_bounded("version", version)?;
        }
        if let Some(summary) = &summary {
            validate_bounded("summary", summary)?;
        }
        if authors.len() > 32 {
            return Err(PackError::manifest("author list exceeds its bound"));
        }
        for author in &authors {
            validate_bounded("author", author)?;
        }
        Ok(Self {
            name,
            version,
            summary,
            authors,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    #[must_use]
    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    #[must_use]
    pub fn authors(&self) -> &[String] {
        &self.authors
    }
}

fn validate_bounded(field: &str, value: &str) -> Result<(), PackError> {
    if value.is_empty() || value.chars().count() > MAX_DISPLAY_STRING_CHARS {
        return Err(PackError::manifest(format!(
            "{field} exceeds its display bound"
        )));
    }
    Ok(())
}
