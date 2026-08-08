use crate::{
    JavaArchitecture, JavaCandidateSource, JavaRequirement, JavaRuntime, discover_java_candidates,
    discovery::path_sort_key, error::java_error, probe_java,
};
use graphene_core::{ErrorCode, Result};
use std::path::PathBuf;

#[must_use]
pub fn is_compatible(
    runtime: &JavaRuntime,
    requirement: &JavaRequirement,
    target: JavaArchitecture,
) -> bool {
    runtime.major_version == requirement.major_version
        && (target == JavaArchitecture::Other || runtime.architecture == target)
}

pub async fn select_java(
    requirement: &JavaRequirement,
    explicit: Option<PathBuf>,
) -> Result<JavaRuntime> {
    if requirement.major_version == 0 {
        return Err(java_error(
            ErrorCode::JavaIncompatible,
            "Java requirement has an invalid major version",
        ));
    }

    let candidates = discover_java_candidates(explicit).await?;
    if candidates.is_empty() {
        return Err(java_error(
            ErrorCode::JavaNotFound,
            "no local Java candidates were found",
        )
        .with_context("required_major", requirement.major_version.to_string()));
    }

    let target = JavaArchitecture::current();
    let candidate_count = candidates.len();
    let mut probed = Vec::<(JavaCandidateSource, JavaRuntime)>::new();

    for candidate in candidates {
        if let Ok(runtime) = probe_java(&candidate).await {
            probed.push((candidate.source, runtime));
        }
    }

    choose_compatible_runtime(requirement, target, candidate_count, probed)
}

pub(crate) fn choose_compatible_runtime(
    requirement: &JavaRequirement,
    target: JavaArchitecture,
    candidate_count: usize,
    probed: Vec<(JavaCandidateSource, JavaRuntime)>,
) -> Result<JavaRuntime> {
    let mut observed = probed
        .iter()
        .map(|(_, runtime)| runtime.major_version)
        .collect::<Vec<_>>();
    let mut compatible = probed
        .into_iter()
        .filter(|(_, runtime)| is_compatible(runtime, requirement, target))
        .collect::<Vec<_>>();
    compatible.sort_by(|(left_source, left), (right_source, right)| {
        left_source
            .priority()
            .cmp(&right_source.priority())
            .then_with(|| path_sort_key(&left.executable).cmp(&path_sort_key(&right.executable)))
    });

    if let Some((_, runtime)) = compatible.into_iter().next() {
        return Ok(runtime);
    }

    observed.sort_unstable();
    observed.dedup();

    Err(java_error(
        ErrorCode::JavaIncompatible,
        "no compatible local Java runtime was found",
    )
    .with_context("required_major", requirement.major_version.to_string())
    .with_context("candidate_count", candidate_count.to_string())
    .with_context(
        "observed_majors",
        observed
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(","),
    )
    .with_context("target_arch", format!("{target:?}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::JavaVendor;

    fn runtime(path: &str, major: u32, architecture: JavaArchitecture) -> JavaRuntime {
        JavaRuntime {
            executable: PathBuf::from(path),
            version: format!("{major}.0.1"),
            major_version: major,
            vendor: JavaVendor::Other("fixture".into()),
            architecture,
            java_home: None,
        }
    }

    #[test]
    fn compatibility_requires_declared_major_and_architecture() {
        let runtime = runtime("java", 21, JavaArchitecture::X86_64);
        let requirement = JavaRequirement {
            major_version: 21,
            component_hint: None,
        };

        assert!(is_compatible(
            &runtime,
            &requirement,
            JavaArchitecture::X86_64
        ));
        assert!(!is_compatible(
            &runtime,
            &JavaRequirement {
                major_version: 17,
                component_hint: None,
            },
            JavaArchitecture::X86_64
        ));
        assert!(!is_compatible(
            &runtime,
            &requirement,
            JavaArchitecture::AArch64
        ));
    }

    #[test]
    fn explicit_override_wins_and_ties_are_path_deterministic() {
        let requirement = JavaRequirement {
            major_version: 21,
            component_hint: None,
        };
        let target = JavaArchitecture::X86_64;
        let selected = choose_compatible_runtime(
            &requirement,
            target,
            3,
            vec![
                (JavaCandidateSource::Path, runtime("/z/java", 21, target)),
                (
                    JavaCandidateSource::Explicit,
                    runtime("/y/java", 21, target),
                ),
                (
                    JavaCandidateSource::Explicit,
                    runtime("/a/java", 21, target),
                ),
            ],
        )
        .expect("selection");
        assert_eq!(selected.executable, PathBuf::from("/a/java"));
    }

    #[test]
    fn incompatible_selection_returns_structured_error() {
        let requirement = JavaRequirement {
            major_version: 21,
            component_hint: None,
        };
        let error = choose_compatible_runtime(
            &requirement,
            JavaArchitecture::AArch64,
            2,
            vec![
                (
                    JavaCandidateSource::Path,
                    runtime("java17", 17, JavaArchitecture::AArch64),
                ),
                (
                    JavaCandidateSource::Path,
                    runtime("java21-x64", 21, JavaArchitecture::X86_64),
                ),
            ],
        )
        .expect_err("must reject");

        assert_eq!(error.code, ErrorCode::JavaIncompatible);
    }
}
