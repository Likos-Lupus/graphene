use crate::archive::{PackPath, limits::MAX_DISPLAY_STRING_CHARS};
use crate::error::PackError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Host-owned optional-file policy. The engine never invents choices on the host's behalf.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OptionalChoicePolicy {
    /// Install required files only; skip every optional choice.
    RequiredOnly,
    /// Install every optional choice.
    IncludeAllOptional,
    /// Install exactly the listed choice ids; missing ids fail planning.
    Explicit(BTreeSet<String>),
}

/// One optional file a host may select.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackOptionalChoice {
    id: String,
    destination: Option<PackPath>,
    label: String,
    size: Option<u64>,
    description: Option<String>,
    default_selected: bool,
}

impl PackOptionalChoice {
    pub fn new(
        id: impl Into<String>,
        destination: PackPath,
        label: impl Into<String>,
        size: u64,
        description: Option<String>,
        default_selected: bool,
    ) -> Result<Self, PackError> {
        let id = id.into();
        let label = label.into();
        if id.is_empty() || id.chars().count() > MAX_DISPLAY_STRING_CHARS {
            return Err(PackError::manifest("optional choice id exceeds its bound"));
        }
        if label.is_empty() || label.chars().count() > MAX_DISPLAY_STRING_CHARS {
            return Err(PackError::manifest(
                "optional choice label exceeds its bound",
            ));
        }
        if description
            .as_ref()
            .is_some_and(|text| text.chars().count() > MAX_DISPLAY_STRING_CHARS)
        {
            return Err(PackError::manifest(
                "optional choice description exceeds its bound",
            ));
        }
        Ok(Self {
            id,
            destination: Some(destination),
            label,
            size: Some(size),
            description,
            default_selected,
        })
    }

    /// Creates a choice whose destination and size become known only after exact provider lookup.
    pub fn unresolved(
        id: impl Into<String>,
        label: impl Into<String>,
        description: Option<String>,
        default_selected: bool,
    ) -> Result<Self, PackError> {
        let id = id.into();
        let label = label.into();
        validate_choice_text(&id, &label, description.as_deref())?;
        Ok(Self {
            id,
            destination: None,
            label,
            size: None,
            description,
            default_selected,
        })
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub const fn destination(&self) -> Option<&PackPath> {
        self.destination.as_ref()
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn size(&self) -> Option<u64> {
        self.size
    }

    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    #[must_use]
    pub const fn default_selected(&self) -> bool {
        self.default_selected
    }
}

fn validate_choice_text(id: &str, label: &str, description: Option<&str>) -> Result<(), PackError> {
    if id.is_empty() || id.chars().count() > MAX_DISPLAY_STRING_CHARS {
        return Err(PackError::manifest("optional choice id exceeds its bound"));
    }
    if label.is_empty() || label.chars().count() > MAX_DISPLAY_STRING_CHARS {
        return Err(PackError::manifest(
            "optional choice label exceeds its bound",
        ));
    }
    if description.is_some_and(|text| text.chars().count() > MAX_DISPLAY_STRING_CHARS) {
        return Err(PackError::manifest(
            "optional choice description exceeds its bound",
        ));
    }
    Ok(())
}
