use serde::{Deserialize, Serialize};

/// Discriminator for the kind of managed content.
/// Phase 5 manages only `Mod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum ContentKind {
    #[default]
    Mod,
}
