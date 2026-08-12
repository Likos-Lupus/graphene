use super::MinecraftVersionPatch;
use crate::{
    ResolvedLibrary, ResolvedMinecraft, RuleContext, error::mc_error, resolution::resolve_library,
};
use graphene_core::{ErrorCode, Result};
use std::collections::{BTreeMap, BTreeSet};

pub fn compose_minecraft(
    mut base: ResolvedMinecraft,
    patches: &[MinecraftVersionPatch],
    context: &RuleContext,
) -> Result<ResolvedMinecraft> {
    let mut component_uids = base
        .components
        .iter()
        .map(|component| component.uid.clone())
        .collect::<BTreeSet<_>>();

    for patch in patches {
        if !component_uids.insert(patch.component.uid.clone()) {
            return Err(mc_error(
                ErrorCode::ComponentPatchInvalid,
                "component patch duplicates an already resolved component",
            )
            .with_context("component_uid", patch.component.uid.to_string()));
        }

        if let Some(main_class) = &patch.main_class {
            if main_class.trim().is_empty() || main_class.len() > 512 || main_class.contains('\0') {
                return Err(mc_error(
                    ErrorCode::ComponentPatchInvalid,
                    "component patch main class is invalid",
                ));
            }
            base.main_class = main_class.clone();
        }

        merge_libraries(&mut base.libraries, &patch.libraries, context)?;
        base.jvm_args.extend(patch.jvm_args.iter().cloned());
        base.game_args.extend(patch.game_args.iter().cloned());

        if let Some(requirement) = &patch.java_requirement
            && base.java_requirement != *requirement
        {
            return Err(mc_error(
                ErrorCode::ComponentJavaConflict,
                "component Java requirement conflicts with the base Minecraft requirement",
            )
            .with_context("component_uid", patch.component.uid.to_string())
            .with_context("base_java", base.java_requirement.major_version.to_string())
            .with_context("component_java", requirement.major_version.to_string()));
        }

        if let Some(logging) = &patch.logging {
            base.logging = Some(logging.clone());
        }
        base.components.push(patch.component.clone());
    }

    validate_final(&base)?;
    Ok(base)
}

fn merge_libraries(
    current: &mut Vec<ResolvedLibrary>,
    additions: &[crate::Library],
    context: &RuleContext,
) -> Result<()> {
    let mut positions = BTreeMap::<String, usize>::new();
    for (index, library) in current.iter().enumerate() {
        positions.insert(library.coordinate.library_identity(), index);
    }

    for library in additions {
        let Some(resolved) = resolve_library(library.clone(), context)? else {
            continue;
        };

        let identity = resolved.coordinate.library_identity();
        if let Some(index) = positions.get(&identity).copied() {
            current[index] = resolved;
        } else {
            positions.insert(identity, current.len());
            current.push(resolved);
        }
    }

    validate_library_destinations(current)
}

fn validate_library_destinations(libraries: &[ResolvedLibrary]) -> Result<()> {
    let mut destinations = BTreeMap::<String, String>::new();
    for library in libraries {
        for artifact in [&library.classpath_artifact, &library.native_artifact]
            .into_iter()
            .flatten()
        {
            let path = artifact.relative_path.as_str().to_owned();
            let identity = library.coordinate.library_identity();

            if let Some(existing) = destinations.insert(path.clone(), identity.clone())
                && existing != identity
            {
                return Err(mc_error(
                    ErrorCode::ComponentPatchInvalid,
                    "component libraries collide at one managed destination",
                )
                .with_context("destination", path));
            }
        }
    }

    Ok(())
}

fn validate_final(minecraft: &ResolvedMinecraft) -> Result<()> {
    if minecraft.main_class.trim().is_empty() || minecraft.main_class.contains('\0') {
        return Err(mc_error(
            ErrorCode::ComponentPatchInvalid,
            "composed Minecraft metadata has an invalid main class",
        ));
    }

    if minecraft.components.is_empty()
        || minecraft
            .components
            .iter()
            .filter(|component| component.kind == crate::ComponentKind::Minecraft)
            .count()
            != 1
    {
        return Err(mc_error(
            ErrorCode::ComponentPatchInvalid,
            "composed Minecraft metadata has an invalid component set",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Argument, ComponentProvenance, ComponentUid, ComponentVersion, Library, MavenCoordinate,
        MinecraftArch, MinecraftJavaRequirement, MinecraftOs, MinecraftVersionId,
        MinecraftVersionType, ResolvedComponent,
        test_support::{assets, context, resolved_artifact},
    };
    use std::collections::BTreeMap;

    fn base() -> ResolvedMinecraft {
        ResolvedMinecraft {
            version_id: MinecraftVersionId::new("1.21.1").expect("version"),
            version_type: MinecraftVersionType::Release,
            main_class: "net.minecraft.client.main.Main".into(),
            client: resolved_artifact(1, ".minecraft/versions/1.21.1/1.21.1.jar"),
            libraries: Vec::new(),
            assets: assets(),
            logging: None,
            jvm_args: Vec::new(),
            game_args: Vec::new(),
            java_requirement: MinecraftJavaRequirement {
                major_version: 21,
                component_hint: None,
            },
            components: vec![ResolvedComponent::minecraft("1.21.1").expect("component")],
        }
    }

    fn loader_component() -> ResolvedComponent {
        ResolvedComponent {
            uid: ComponentUid::new("net.fabricmc.fabric-loader").expect("uid"),
            version: ComponentVersion::new("0.16.14").expect("version"),
            kind: crate::ComponentKind::Loader,
            provenance: ComponentProvenance::new("fabric", None).expect("provenance"),
        }
    }

    #[test]
    fn scalar_and_arguments_compose_without_synthetic_base_version() {
        let mut patch = MinecraftVersionPatch::empty(loader_component());
        patch.main_class = Some("net.fabricmc.loader.impl.launch.knot.KnotClient".into());
        patch
            .jvm_args
            .push(Argument::Literal("-Dfabric=true".into()));
        let resolved = compose_minecraft(
            base(),
            &[patch],
            &context(MinecraftOs::Linux, MinecraftArch::X86_64),
        )
        .expect("compose");

        assert_eq!(resolved.version_id.as_str(), "1.21.1");
        assert_eq!(resolved.components.len(), 2);
        assert_eq!(
            resolved.jvm_args,
            vec![Argument::Literal("-Dfabric=true".into())]
        );
    }

    #[test]
    fn library_replacement_uses_maven_identity() {
        let mut base = base();
        base.libraries.push(ResolvedLibrary {
            coordinate: MavenCoordinate::parse("org.example:demo:1").expect("coordinate"),
            classpath_artifact: Some(resolved_artifact(
                2,
                "shared/libraries/org/example/demo/1/demo-1.jar",
            )),
            native_artifact: None,
        });

        let mut patch = MinecraftVersionPatch::empty(loader_component());
        patch.libraries.push(Library {
            coordinate: MavenCoordinate::parse("org.example:demo:2").expect("coordinate"),
            rules: Vec::new(),
            artifact: Some(resolved_artifact(
                3,
                "shared/libraries/org/example/demo/2/demo-2.jar",
            )),
            classifiers: BTreeMap::new(),
            natives: BTreeMap::new(),
        });

        let resolved = compose_minecraft(
            base,
            &[patch],
            &context(MinecraftOs::Linux, MinecraftArch::X86_64),
        )
        .expect("compose");
        assert_eq!(resolved.libraries.len(), 1);
        assert_eq!(resolved.libraries[0].coordinate.version, "2");
    }

    #[test]
    fn incompatible_java_requirement_fails_structurally() {
        let mut patch = MinecraftVersionPatch::empty(loader_component());
        patch.java_requirement = Some(MinecraftJavaRequirement {
            major_version: 17,
            component_hint: None,
        });

        assert_eq!(
            compose_minecraft(
                base(),
                &[patch],
                &context(MinecraftOs::Linux, MinecraftArch::X86_64)
            )
            .expect_err("conflict")
            .code,
            ErrorCode::ComponentJavaConflict
        );
    }
}
