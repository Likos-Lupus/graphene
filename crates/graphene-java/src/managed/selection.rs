use super::ManagedJavaRuntime;
use crate::{JavaArchitecture, JavaRequirement};

#[must_use]
pub fn select_managed_runtime(
    requirement: &JavaRequirement,
    target: JavaArchitecture,
    runtimes: &[ManagedJavaRuntime],
) -> Option<ManagedJavaRuntime> {
    let mut compatible = runtimes
        .iter()
        .filter(|runtime| {
            runtime.major_version == requirement.major_version
                && (target == JavaArchitecture::Other || runtime.architecture == target)
                && runtime.validate().is_ok()
        })
        .cloned()
        .collect::<Vec<_>>();
    compatible.sort_by(|left, right| {
        version_key(&right.version)
            .cmp(&version_key(&left.version))
            .then_with(|| left.id.to_string().cmp(&right.id.to_string()))
    });
    compatible.into_iter().next()
}

fn version_key(value: &str) -> Vec<u32> {
    value
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{JavaImageKind, JavaOperatingSystem, JavaVendor, MANAGED_RUNTIME_SCHEMA_VERSION};
    use graphene_core::{ManagedRuntimeId, Sha256Digest};

    fn runtime(version: &str, id: u8) -> ManagedJavaRuntime {
        ManagedJavaRuntime {
            schema_version: MANAGED_RUNTIME_SCHEMA_VERSION,
            id: ManagedRuntimeId::from_bytes([id; 16]),
            provider: "fixture".into(),
            distribution: "fixture".into(),
            release_name: version.into(),
            version: version.into(),
            major_version: 21,
            vendor: JavaVendor::Adoptium,
            os: JavaOperatingSystem::Linux,
            architecture: JavaArchitecture::X86_64,
            image: JavaImageKind::Jre,
            archive_sha256: Sha256Digest::from_bytes([id; 32]),
            executable_relative_path: "runtime/bin/java".into(),
        }
    }

    #[test]
    fn newest_compatible_committed_runtime_wins_deterministically() {
        let selected = select_managed_runtime(
            &JavaRequirement {
                major_version: 21,
                component_hint: None,
            },
            JavaArchitecture::X86_64,
            &[runtime("21.0.9+10", 1), runtime("21.0.10+7", 2)],
        )
        .unwrap();
        assert_eq!(selected.version, "21.0.10+7");
    }
}
