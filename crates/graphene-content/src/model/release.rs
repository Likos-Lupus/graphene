use serde::{Deserialize, Serialize};

/// Normalized release channel for a content version.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReleaseChannel {
    /// Highest stability official release.
    #[default]
    Release,
    /// Public beta release.
    Beta,
    /// Early alpha or development preview release.
    Alpha,
    /// Unknown or unclassified channel.
    Unknown,
}
