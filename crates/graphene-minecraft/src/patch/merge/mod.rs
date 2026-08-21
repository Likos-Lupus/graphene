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
mod tests;
