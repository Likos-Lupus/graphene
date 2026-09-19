use super::finding::FindingId;
use graphene_core::{DiagnosticCode, DiagnosticParameters};
use serde::{Deserialize, Serialize};

/// Non-mutating, host-independent next action a recommendation points at.
///
/// A recommendation never authorizes the action; the caller must explicitly invoke the
/// corresponding existing Graphene service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecommendationActionKind {
    RunQuickVerification,
    RunFullVerification,
    PlanInstanceRepair,
    SelectCompatibleJava,
    InstallManagedJava,
    ReviewMemoryConfiguration,
    ReviewContentConflict,
    ReviewMissingContentDependency,
    ReviewGraphicsOrNativeEnvironment,
    ReauthenticateIfLaunchRequiresIt,
    CollectAdditionalEvidence,
    NoAutomaticRemediation,
}

impl RecommendationActionKind {
    /// Stable rank used for deterministic recommendation ordering.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::PlanInstanceRepair => 0,
            Self::SelectCompatibleJava => 1,
            Self::InstallManagedJava => 2,
            Self::RunFullVerification => 3,
            Self::RunQuickVerification => 4,
            Self::ReviewContentConflict => 5,
            Self::ReviewMissingContentDependency => 6,
            Self::ReviewGraphicsOrNativeEnvironment => 7,
            Self::ReviewMemoryConfiguration => 8,
            Self::ReauthenticateIfLaunchRequiresIt => 9,
            Self::CollectAdditionalEvidence => 10,
            Self::NoAutomaticRemediation => 11,
        }
    }
}

/// An explainable recommendation that converges on an existing Graphene capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticRecommendation {
    pub code: DiagnosticCode,
    pub action: RecommendationActionKind,
    pub parameters: DiagnosticParameters,
    pub findings: Vec<FindingId>,
}

impl DiagnosticRecommendation {
    /// Creates a recommendation with no related findings.
    #[must_use]
    pub fn new(
        code: DiagnosticCode,
        action: RecommendationActionKind,
        parameters: DiagnosticParameters,
    ) -> Self {
        Self {
            code,
            action,
            parameters,
            findings: Vec::new(),
        }
    }

    /// Attaches related finding identifiers.
    #[must_use]
    pub fn with_findings(mut self, findings: impl IntoIterator<Item = FindingId>) -> Self {
        self.findings = findings.into_iter().collect();
        self
    }

    fn order_key(&self) -> (u8, &str, &DiagnosticParameters, &[FindingId]) {
        (
            self.action.rank(),
            self.code.as_str(),
            &self.parameters,
            &self.findings,
        )
    }
}

pub(crate) fn normalize_recommendations(recommendations: &mut Vec<DiagnosticRecommendation>) {
    recommendations.sort_by(|left, right| left.order_key().cmp(&right.order_key()));
    recommendations.dedup();
}
