use crate::model::{
    DiagnosticFinding, DiagnosticRecommendation, FindingId, RecommendationActionKind, codes,
};
use graphene_core::{DiagnosticCode, DiagnosticParameters};
use std::collections::BTreeMap;

fn recommendation_code(action: RecommendationActionKind) -> &'static str {
    match action {
        RecommendationActionKind::RunQuickVerification => "RECOMMEND_RUN_QUICK_VERIFICATION",
        RecommendationActionKind::RunFullVerification => "RECOMMEND_RUN_FULL_VERIFICATION",
        RecommendationActionKind::PlanInstanceRepair => "RECOMMEND_PLAN_INSTANCE_REPAIR",
        RecommendationActionKind::SelectCompatibleJava => "RECOMMEND_SELECT_COMPATIBLE_JAVA",
        RecommendationActionKind::InstallManagedJava => "RECOMMEND_INSTALL_MANAGED_JAVA",
        RecommendationActionKind::ReviewMemoryConfiguration => {
            "RECOMMEND_REVIEW_MEMORY_CONFIGURATION"
        }
        RecommendationActionKind::ReviewContentConflict => "RECOMMEND_REVIEW_CONTENT_CONFLICT",
        RecommendationActionKind::ReviewMissingContentDependency => {
            "RECOMMEND_REVIEW_MISSING_CONTENT_DEPENDENCY"
        }
        RecommendationActionKind::ReviewGraphicsOrNativeEnvironment => {
            "RECOMMEND_REVIEW_GRAPHICS_OR_NATIVE_ENVIRONMENT"
        }
        RecommendationActionKind::ReauthenticateIfLaunchRequiresIt => {
            "RECOMMEND_REAUTHENTICATE_IF_LAUNCH_REQUIRES_IT"
        }
        RecommendationActionKind::CollectAdditionalEvidence => {
            "RECOMMEND_COLLECT_ADDITIONAL_EVIDENCE"
        }
        RecommendationActionKind::NoAutomaticRemediation => "RECOMMEND_NO_AUTOMATIC_REMEDIATION",
    }
}

fn actions_for(code: &str) -> &'static [RecommendationActionKind] {
    use RecommendationActionKind::{
        CollectAdditionalEvidence, InstallManagedJava, NoAutomaticRemediation, PlanInstanceRepair,
        ReviewContentConflict, ReviewGraphicsOrNativeEnvironment, ReviewMemoryConfiguration,
        ReviewMissingContentDependency, SelectCompatibleJava,
    };

    match code {
        codes::MANAGED_FILE_MISSING
        | codes::MANAGED_FILE_WRONG_TYPE
        | codes::MANAGED_FILE_SIZE_MISMATCH
        | codes::MANAGED_FILE_HASH_MISMATCH
        | codes::GENERATED_OUTPUT_MISSING
        | codes::GENERATED_OUTPUT_MISMATCH
        | codes::NATIVE_DIRECTORY_MISSING
        | codes::INSTANCE_IDENTITY_MISMATCH
        | codes::REPAIR_SOURCE_UNAVAILABLE
        | codes::REPAIR_CONFLICT
        | codes::LOCKFILE_MISSING
        | codes::LOCKFILE_INVALID
        | codes::INSTALL_RECEIPT_INVALID
        | codes::LEGACY_DESIRED_STATE => &[PlanInstanceRepair],
        codes::UNSAFE_SYMLINK => &[PlanInstanceRepair, CollectAdditionalEvidence],
        codes::INSTANCE_DESCRIPTOR_INVALID => &[CollectAdditionalEvidence, NoAutomaticRemediation],
        codes::JAVA_CLASS_VERSION_MISMATCH | codes::JAVA_PROBE_FAILED => &[SelectCompatibleJava],
        "JAVA_NOT_FOUND" => &[InstallManagedJava, SelectCompatibleJava],
        "JAVA_VERSION_TOO_OLD" | "JAVA_VERSION_TOO_NEW" | "JAVA_ARCHITECTURE_MISMATCH" => {
            &[SelectCompatibleJava]
        }
        codes::JVM_OUT_OF_MEMORY => &[ReviewMemoryConfiguration],
        codes::DUPLICATE_MOD_ID => &[ReviewContentConflict],
        codes::LOADER_MISSING_DEPENDENCY | codes::MISSING_REQUIRED_DEPENDENCY => {
            &[ReviewMissingContentDependency]
        }
        codes::CONTENT_INCOMPATIBILITY | codes::LOADER_ENVIRONMENT_MISMATCH => {
            &[ReviewContentConflict]
        }
        codes::MALFORMED_MOD_ARCHIVE => &[ReviewContentConflict, CollectAdditionalEvidence],
        codes::NATIVE_LIBRARY_FAILURE | codes::GRAPHICS_INITIALIZATION_FAILURE => {
            &[ReviewGraphicsOrNativeEnvironment]
        }
        codes::MIXIN_FAILURE
        | codes::CLASS_LOADING_FAILURE
        | codes::LOADER_LOAD_FAILURE
        | codes::JVM_FATAL_ERROR
        | codes::PROCESS_EXIT_NONZERO
        | codes::CAUSE_UNDETERMINED => &[CollectAdditionalEvidence],
        _ => &[],
    }
}

/// Derives deduplicated, deterministically ordered recommendations that converge on existing
/// Graphene capabilities. Recommendations never authorize automatic mutation.
#[must_use]
pub fn derive_recommendations(findings: &[DiagnosticFinding]) -> Vec<DiagnosticRecommendation> {
    let mut grouped: BTreeMap<RecommendationActionKind, Vec<FindingId>> = BTreeMap::new();
    for finding in findings {
        for action in actions_for(finding.diagnostic.code.as_str()) {
            grouped.entry(*action).or_default().push(finding.id);
        }
    }

    grouped
        .into_iter()
        .map(|(action, mut findings)| {
            findings.sort();
            findings.dedup();
            let mut parameters = DiagnosticParameters::new();
            parameters.insert("finding_count".to_owned(), findings.len().to_string());
            DiagnosticRecommendation::new(
                DiagnosticCode::new(recommendation_code(action)),
                action,
                parameters,
            )
            .with_findings(findings)
        })
        .collect()
}
