use crate::{
    Argument, MinecraftJavaRequirement, MinecraftVersionId, MinecraftVersionMetadata,
    MinecraftVersionType, ResolvedArtifact, ResolvedAssets, ResolvedComponent, ResolvedLibrary,
    ResolvedLogging, RuleContext, error::mc_error, rules_allow, tokenize_legacy_arguments,
};
use graphene_core::{ErrorCode, Result};
use serde::{Deserialize, Serialize};

/// Stable provider-neutral fully resolved Minecraft launch/install model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedMinecraft {
    pub version_id: MinecraftVersionId,
    pub version_type: MinecraftVersionType,
    pub main_class: String,
    pub client: ResolvedArtifact,
    pub libraries: Vec<ResolvedLibrary>,
    pub assets: ResolvedAssets,
    pub logging: Option<ResolvedLogging>,
    pub jvm_args: Vec<Argument>,
    pub game_args: Vec<Argument>,
    pub java_requirement: MinecraftJavaRequirement,
    pub components: Vec<ResolvedComponent>,
}

/// Converts complete inherited metadata plus a parsed asset index into the Tier A domain result.
pub fn resolve_minecraft(
    metadata: MinecraftVersionMetadata,
    assets: ResolvedAssets,
    context: &RuleContext,
) -> Result<ResolvedMinecraft> {
    if metadata.inherits_from.is_some() {
        return Err(mc_error(
            ErrorCode::MinecraftMetadataInvalid,
            "Minecraft inheritance remained unresolved",
        ));
    }

    let main_class = metadata
        .main_class
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            mc_error(
                ErrorCode::MinecraftMetadataUnsupported,
                "Tier A metadata is missing a main class",
            )
        })?;
    let client = metadata.client.ok_or_else(|| {
        mc_error(
            ErrorCode::MinecraftMetadataUnsupported,
            "Tier A metadata is missing a client artifact",
        )
    })?;
    let java_requirement = metadata.java_requirement.ok_or_else(|| {
        mc_error(
            ErrorCode::MinecraftMetadataUnsupported,
            "Tier A metadata is missing a Java requirement",
        )
    })?;

    if java_requirement.major_version == 0 {
        return Err(mc_error(
            ErrorCode::MinecraftMetadataInvalid,
            "Java major requirement must be non-zero",
        ));
    }

    let mut libraries = Vec::new();
    for library in metadata.libraries {
        if let Some(library) = resolve_library(library, context)? {
            libraries.push(library);
        }
    }

    let game_args = if metadata.game_args.is_empty() {
        match metadata.legacy_minecraft_arguments {
            Some(text) => tokenize_legacy_arguments(&text)?
                .into_iter()
                .map(Argument::Literal)
                .collect(),
            None => Vec::new(),
        }
    } else {
        metadata.game_args
    };

    let base_component = ResolvedComponent::minecraft(metadata.id.as_str())?;

    Ok(ResolvedMinecraft {
        version_id: metadata.id,
        version_type: metadata.version_type,
        main_class,
        client,
        libraries,
        assets,
        logging: metadata.logging,
        jvm_args: metadata.jvm_args,
        game_args,
        java_requirement,
        components: vec![base_component],
    })
}

pub(crate) fn resolve_library(
    library: crate::Library,
    context: &RuleContext,
) -> Result<Option<ResolvedLibrary>> {
    if !rules_allow(&library.rules, context)? {
        return Ok(None);
    }

    let native_artifact = match library.natives.get(&context.os) {
        Some(template) => {
            let classifier = template.replace("${arch}", context.arch.classifier_value());
            Some(
                library
                    .classifiers
                    .get(&classifier)
                    .cloned()
                    .ok_or_else(|| {
                        mc_error(
                            ErrorCode::MinecraftNativeUnavailable,
                            "selected native classifier has no declared download",
                        )
                        .with_context("library", library.coordinate.library_identity())
                        .with_context("classifier", classifier)
                    })?,
            )
        }
        None => None,
    };

    Ok(Some(ResolvedLibrary {
        coordinate: library.coordinate,
        classpath_artifact: library.artifact,
        native_artifact,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{assets, complete_metadata, context, resolved_artifact};
    use crate::{Library, MavenCoordinate, MinecraftArch, MinecraftOs};
    use std::collections::BTreeMap;

    #[test]
    fn resolution_selects_classpath_and_native_classifier() {
        let classpath = resolved_artifact(2, "shared/libraries/org/example/demo/1/demo-1.jar");
        let native = resolved_artifact(
            3,
            "shared/libraries/org/example/demo/1/demo-1-natives-linux-64.jar",
        );
        let library = Library {
            coordinate: MavenCoordinate::parse("org.example:demo:1").expect("coordinate"),
            rules: Vec::new(),
            artifact: Some(classpath.clone()),
            classifiers: BTreeMap::from([("natives-linux-64".into(), native.clone())]),
            natives: BTreeMap::from([(MinecraftOs::Linux, "natives-linux-${arch}".into())]),
        };
        let resolved = resolve_minecraft(
            complete_metadata(vec![library]),
            assets(),
            &context(MinecraftOs::Linux, MinecraftArch::X86_64),
        )
        .expect("resolved");

        assert_eq!(resolved.libraries.len(), 1);
        assert_eq!(
            resolved.libraries[0].classpath_artifact.as_ref(),
            Some(&classpath)
        );
        assert_eq!(
            resolved.libraries[0].native_artifact.as_ref(),
            Some(&native)
        );
    }

    #[test]
    fn resolution_rejects_missing_selected_native_classifier() {
        let library = Library {
            coordinate: MavenCoordinate::parse("org.example:demo:1").expect("coordinate"),
            rules: Vec::new(),
            artifact: Some(resolved_artifact(
                2,
                "shared/libraries/org/example/demo/1/demo-1.jar",
            )),
            classifiers: BTreeMap::new(),
            natives: BTreeMap::from([(MinecraftOs::Linux, "natives-linux-${arch}".into())]),
        };
        let error = resolve_minecraft(
            complete_metadata(vec![library]),
            assets(),
            &context(MinecraftOs::Linux, MinecraftArch::X86_64),
        )
        .expect_err("missing native classifier");

        assert_eq!(error.code, ErrorCode::MinecraftNativeUnavailable);
    }
}
