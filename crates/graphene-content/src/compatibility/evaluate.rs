use crate::model::{
    compatibility::{
        CompatibilityResult, IncompatibilityReason, InstanceContentContext, ReleaseChannelPolicy,
    },
    version::ContentVersion,
};

/// Evaluates whether a content version is compatible with the given instance context and release policy.
#[must_use]
pub fn evaluate_compatibility(
    version: &ContentVersion,
    context: &InstanceContentContext,
    policy: ReleaseChannelPolicy,
) -> CompatibilityResult {
    let mut reasons = Vec::new();

    if !version.available {
        reasons.push(IncompatibilityReason::ContentUnavailable);
    }

    // 1. Release channel policy check
    if !policy.allows(version.release_channel) {
        reasons.push(IncompatibilityReason::ReleaseChannelMismatch {
            declared: version.release_channel,
            policy,
        });
    }

    // 2. Minecraft version compatibility
    if !version.game_versions.is_empty()
        && !version
            .game_versions
            .iter()
            .any(|gv| gv == &context.minecraft_version)
    {
        reasons.push(IncompatibilityReason::MinecraftVersionMismatch {
            declared: version.game_versions.clone(),
            expected: context.minecraft_version.clone(),
        });
    }

    // 3. Loader compatibility
    if let Some(expected_loader) = context.loader {
        if !version.loaders.is_empty() && !version.loaders.contains(&expected_loader) {
            reasons.push(IncompatibilityReason::LoaderMismatch {
                declared: version.loaders.clone(),
                expected: Some(expected_loader),
            });
        }
    } else if !version.loaders.is_empty() {
        // Vanilla instance but mod declared specific loaders
        reasons.push(IncompatibilityReason::LoaderMismatch {
            declared: version.loaders.clone(),
            expected: None,
        });
    }

    // 4. Client environment support
    if !version.environment.supports_client() {
        reasons.push(IncompatibilityReason::EnvironmentMismatch {
            declared: version.environment,
        });
    }

    // 5. Downloadable and verifiable file check
    if version.primary_file().is_none() {
        reasons.push(IncompatibilityReason::NoVerifiableFile);
    }

    if reasons.is_empty() {
        CompatibilityResult::Compatible
    } else {
        CompatibilityResult::Incompatible(reasons)
    }
}
