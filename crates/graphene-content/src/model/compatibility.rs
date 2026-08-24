use crate::model::{release::ReleaseChannel, version::EnvironmentSupport};
use graphene_core::InstanceId;
use graphene_minecraft::LoaderKind;
use serde::{Deserialize, Serialize};

/// Target side for content evaluation (Phase 5 is launcher/client side).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContentSide {
    #[default]
    Client,
}

/// Instance context derived from committed components for content compatibility checks.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstanceContentContext {
    pub instance_id: InstanceId,
    pub minecraft_version: String,
    pub loader: Option<LoaderKind>,
    pub exact_loader_version: Option<String>,
    pub side: ContentSide,
}

impl InstanceContentContext {
    #[must_use]
    pub fn new(
        instance_id: InstanceId,
        minecraft_version: impl Into<String>,
        loader: Option<LoaderKind>,
        exact_loader_version: Option<String>,
    ) -> Self {
        Self {
            instance_id,
            minecraft_version: minecraft_version.into(),
            loader,
            exact_loader_version,
            side: ContentSide::Client,
        }
    }
}

/// Explicit release channel preference policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReleaseChannelPolicy {
    /// Only consider official full releases.
    ReleaseOnly,
    /// Accept releases and public betas.
    #[default]
    ReleaseOrBeta,
    /// Accept any channel including early alphas.
    Any,
}

impl ReleaseChannelPolicy {
    #[must_use]
    pub const fn allows(&self, channel: ReleaseChannel) -> bool {
        match self {
            Self::ReleaseOnly => matches!(channel, ReleaseChannel::Release),
            Self::ReleaseOrBeta => {
                matches!(channel, ReleaseChannel::Release | ReleaseChannel::Beta)
            }
            Self::Any => true,
        }
    }
}

/// Reason why a content version is incompatible with an instance context or policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IncompatibilityReason {
    MinecraftVersionMismatch {
        declared: Vec<String>,
        expected: String,
    },
    LoaderMismatch {
        declared: Vec<LoaderKind>,
        expected: Option<LoaderKind>,
    },
    EnvironmentMismatch {
        declared: EnvironmentSupport,
    },
    ReleaseChannelMismatch {
        declared: ReleaseChannel,
        policy: ReleaseChannelPolicy,
    },
    NoVerifiableFile,
    ContentUnavailable,
}

/// Result of evaluating a content version against an instance context and policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompatibilityResult {
    Compatible,
    Incompatible(Vec<IncompatibilityReason>),
}

impl CompatibilityResult {
    #[must_use]
    pub const fn is_compatible(&self) -> bool {
        matches!(self, Self::Compatible)
    }
}
