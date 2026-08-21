use super::{config::FabricProviderConfig, dto::ProfileDto};
use crate::loader::common::{
    MAX_PROFILE_LIBRARIES, artifact, join_base, loader_error, validate_endpoint,
};
use graphene_core::{ArtifactIntegrity, ErrorCode, Result};
use graphene_minecraft::{
    Argument, ComponentConflict, ComponentDescriptor, ComponentKind, ComponentProvenance,
    ComponentRequirement, ComponentUid, ComponentVersion, Library, LoaderKind, LoaderSupport,
    LoaderVersion, ManagedPath, MavenCoordinate, MinecraftVersionId, MinecraftVersionPatch,
    ResolvedArtifact, ResolvedComponent,
};
use std::collections::BTreeMap;

pub(super) fn normalize_profile(
    dto: ProfileDto,
    minecraft: &MinecraftVersionId,
    version: &LoaderVersion,
    config: &FabricProviderConfig,
) -> Result<graphene_minecraft::ResolvedLoader> {
    if dto.inherits_from != minecraft.as_str() {
        return Err(loader_error(
            ErrorCode::LoaderProfileInvalid,
            "Fabric profile does not inherit from the requested Minecraft version",
        ));
    }

    if dto.main_class.trim().is_empty()
        || dto.main_class.len() > 512
        || dto.main_class.contains('\0')
    {
        return Err(loader_error(
            ErrorCode::LoaderProfileInvalid,
            "Fabric profile main class is invalid",
        ));
    }

    if dto.libraries.len() > MAX_PROFILE_LIBRARIES {
        return Err(loader_error(
            ErrorCode::LoaderProfileInvalid,
            "Fabric profile contains too many libraries",
        ));
    }

    let mut libraries = Vec::with_capacity(dto.libraries.len());
    for library in dto.libraries {
        let coordinate = MavenCoordinate::parse(&library.name).map_err(|source| {
            loader_error(
                ErrorCode::LoaderProfileInvalid,
                "Fabric profile contains an invalid Maven coordinate",
            )
            .with_source(source)
        })?;
        let path = coordinate.repository_path()?;
        let base = library
            .url
            .unwrap_or_else(|| config.default_maven_base.clone());
        validate_endpoint(&base, config.allow_http)?;
        let url = join_base(&base, path.as_str());
        let mut integrity = ArtifactIntegrity::none();

        if let Some(value) = library.sha1 {
            integrity = integrity.with_sha1(value.parse().map_err(|source| {
                loader_error(
                    ErrorCode::LoaderProfileInvalid,
                    "Fabric library SHA-1 is invalid",
                )
                .with_source(source)
            })?);
        }

        if let Some(value) = library.sha256 {
            integrity = integrity.with_sha256(value.parse().map_err(|source| {
                loader_error(
                    ErrorCode::LoaderProfileInvalid,
                    "Fabric library SHA-256 is invalid",
                )
                .with_source(source)
            })?);
        }

        let resolved = ResolvedArtifact {
            artifact: artifact(url, integrity, library.size, config.allow_http)?,
            relative_path: ManagedPath::new(format!("shared/libraries/{}", path.as_str()))?,
        };

        libraries.push(Library {
            coordinate,
            rules: Vec::new(),
            artifact: Some(resolved),
            classifiers: BTreeMap::new(),
            natives: BTreeMap::new(),
        });
    }

    let jvm_args = normalize_arguments(dto.arguments.jvm)?;
    let game_args = normalize_arguments(dto.arguments.game)?;
    let uid = ComponentUid::new(LoaderKind::Fabric.component_uid())?;
    let component_version = ComponentVersion::new(version.as_str())?;
    let provenance = ComponentProvenance::new("fabric-meta", None)?;
    let resolved_component = ResolvedComponent {
        uid: uid.clone(),
        version: component_version.clone(),
        kind: ComponentKind::Loader,
        provenance,
    };

    let descriptor = ComponentDescriptor {
        uid,
        version: component_version,
        kind: ComponentKind::Loader,
        order: 100,
        requires: vec![ComponentRequirement::Exact {
            uid: ComponentUid::new("net.minecraft")?,
            version: ComponentVersion::new(minecraft.as_str())?,
        }],
        conflicts: vec![
            ComponentConflict {
                uid: ComponentUid::new(LoaderKind::Forge.component_uid())?,
            },
            ComponentConflict {
                uid: ComponentUid::new(LoaderKind::NeoForge.component_uid())?,
            },
        ],
    };

    let mut patch = MinecraftVersionPatch::empty(resolved_component);
    patch.main_class = Some(dto.main_class);
    patch.libraries = libraries;
    patch.jvm_args = jvm_args;
    patch.game_args = game_args;

    Ok(graphene_minecraft::ResolvedLoader {
        kind: LoaderKind::Fabric,
        version: version.clone(),
        minecraft: minecraft.clone(),
        component: descriptor,
        patch,
        preparation: graphene_minecraft::ComponentPreparationRecipe::default(),
        support: LoaderSupport::Supported,
    })
}

fn normalize_arguments(values: Vec<serde_json::Value>) -> Result<Vec<Argument>> {
    if values.len() > 4096 {
        return Err(loader_error(
            ErrorCode::LoaderProfileInvalid,
            "Fabric profile contains too many arguments",
        ));
    }

    values
        .into_iter()
        .map(|value| match value {
            serde_json::Value::String(value)
                if value.len() <= 16 * 1024 && !value.contains('\0') =>
            {
                Ok(Argument::Literal(value))
            }
            _ => Err(loader_error(
                ErrorCode::LoaderProfileInvalid,
                "Fabric profile contains unsupported argument data",
            )),
        })
        .collect()
}

#[cfg(test)]
mod tests;
