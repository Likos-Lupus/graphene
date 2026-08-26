use serde::{Deserialize, Serialize};

/// Supported pack container formats after normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PackFormat {
    Modrinth,
    CurseForge,
    PrismMultiMc,
    Graphene,
    Generic,
}

impl PackFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Modrinth => "modrinth",
            Self::CurseForge => "curseforge",
            Self::PrismMultiMc => "prism-multimc",
            Self::Graphene => "graphene",
            Self::Generic => "generic",
        }
    }
}
