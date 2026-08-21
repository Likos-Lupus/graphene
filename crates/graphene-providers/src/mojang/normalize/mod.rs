use super::{
    artifact::{artifact_for, client_path, parse_sha1, parse_sha1_for, resolved_download},
    config::MojangProviderConfig,
    dto::*,
    error::mc_error,
    policy::{bounded_owned, bounded_string, normalized_source_url},
};
use graphene_core::{ArtifactIntegrity, ArtifactKind, ErrorCode, GrapheneError, Result};
use graphene_minecraft::{
    Argument, LatestVersions, Library, ManagedPath, MavenCoordinate, MinecraftArch,
    MinecraftJavaRequirement, MinecraftOs, MinecraftVersionId, MinecraftVersionMetadata,
    MinecraftVersionType, OsRule, ResolvedArtifact, ResolvedAssetObject, ResolvedAssets,
    ResolvedLogging, Rule, RuleAction, VersionManifest, VersionSummary,
};
use std::collections::{BTreeMap, BTreeSet};

const MAX_MANIFEST_ENTRIES: usize = 50_000;
const MAX_LIBRARIES: usize = 20_000;
const MAX_ASSET_OBJECTS: usize = 500_000;
const MAX_ARGUMENTS: usize = 20_000;

pub(super) fn normalize_manifest(
    bytes: &[u8],
    config: &MojangProviderConfig,
) -> Result<VersionManifest> {
    let dto: ManifestDto = serde_json::from_slice(bytes).map_err(|source| {
        mc_error(
            ErrorCode::MinecraftManifestInvalid,
            "version manifest JSON is invalid",
        )
        .with_source(source)
    })?;

    if dto.versions.len() > MAX_MANIFEST_ENTRIES {
        return Err(mc_error(
            ErrorCode::MinecraftManifestInvalid,
            "version manifest contains too many entries",
        ));
    }

    let latest = LatestVersions {
        release: MinecraftVersionId::new(dto.latest.release)?,
        snapshot: MinecraftVersionId::new(dto.latest.snapshot)?,
    };
    let mut ids = BTreeSet::new();
    let mut versions = Vec::with_capacity(dto.versions.len());

    for version in dto.versions {
        let id = MinecraftVersionId::new(version.id)?;
        if !ids.insert(id.clone()) {
            return Err(mc_error(
                ErrorCode::MinecraftManifestInvalid,
                "version manifest contains a duplicate ID",
            )
            .with_context("version_id", id.to_string()));
        }

        bounded_string(&version.url, "version metadata URL")?;
        let version_url = normalized_source_url(&version.url, config)?;
        let integrity = match version.sha1 {
            Some(hash) => ArtifactIntegrity::none().with_sha1(parse_sha1(&hash)?),
            None => ArtifactIntegrity::none(),
        };
        let artifact = artifact_for(
            ArtifactKind::Metadata,
            version_url,
            integrity,
            version.size,
            config.allow_http,
        )?;

        versions.push(VersionSummary {
            id,
            version_type: MinecraftVersionType::from_provider(&version.version_type),
            metadata: artifact,
            release_time: version.release_time,
            compliance_level: version.compliance_level,
        });
    }

    Ok(VersionManifest { latest, versions })
}

pub(super) fn normalize_version_metadata(
    bytes: &[u8],
    config: &MojangProviderConfig,
) -> Result<MinecraftVersionMetadata> {
    let dto: VersionDto = serde_json::from_slice(bytes).map_err(|source| {
        mc_error(
            ErrorCode::MinecraftMetadataInvalid,
            "version metadata JSON is invalid",
        )
        .with_source(source)
    })?;

    if dto.libraries.len() > MAX_LIBRARIES {
        return Err(mc_error(
            ErrorCode::MinecraftMetadataInvalid,
            "version metadata contains too many libraries",
        ));
    }

    let id = MinecraftVersionId::new(dto.id)?;
    let client = dto
        .downloads
        .and_then(|downloads| downloads.client)
        .map(|download| {
            resolved_download(download, client_path(&id)?, ArtifactKind::Binary, config)
        })
        .transpose()?;
    let (jvm_args, game_args) = match dto.arguments {
        Some(arguments) => {
            if arguments.jvm.len().saturating_add(arguments.game.len()) > MAX_ARGUMENTS {
                return Err(mc_error(
                    ErrorCode::MinecraftArgumentInvalid,
                    "version metadata contains too many launch arguments",
                ));
            }
            (
                normalize_arguments(arguments.jvm)?,
                normalize_arguments(arguments.game)?,
            )
        }
        None => (Vec::new(), Vec::new()),
    };

    let mut libraries = Vec::with_capacity(dto.libraries.len());
    for dto_library in dto.libraries {
        bounded_string(&dto_library.name, "library coordinate")?;
        let coordinate = MavenCoordinate::parse(&dto_library.name)?;
        let rules = normalize_rules(dto_library.rules)?;
        let mut artifact = None;
        let mut classifiers = BTreeMap::new();

        if let Some(downloads) = dto_library.downloads {
            if let Some(download) = downloads.artifact {
                let path = download
                    .path
                    .clone()
                    .map_or_else(|| coordinate.repository_path(), ManagedPath::new)?;
                artifact = Some(resolved_download(
                    download,
                    ManagedPath::new(format!("shared/libraries/{}", path.as_str()))?,
                    ArtifactKind::Binary,
                    config,
                )?);
            }

            for (classifier, download) in downloads.classifiers {
                bounded_string(&classifier, "library classifier")?;
                let path = download.path.clone().ok_or_else(|| {
                    mc_error(
                        ErrorCode::MinecraftLibraryInvalid,
                        "native classifier download is missing its repository path",
                    )
                })?;
                classifiers.insert(
                    classifier,
                    resolved_download(
                        download,
                        ManagedPath::new(format!("shared/libraries/{path}"))?,
                        ArtifactKind::Binary,
                        config,
                    )?,
                );
            }
        }

        let mut natives = BTreeMap::new();
        for (os, classifier) in dto_library.natives {
            bounded_string(&classifier, "native classifier template")?;
            natives.insert(normalize_os(&os), classifier);
        }

        libraries.push(Library {
            coordinate,
            rules,
            artifact,
            classifiers,
            natives,
        });
    }

    let asset_index = dto
        .asset_index
        .map(|index| {
            let index_id = bounded_owned(index.id.clone(), "asset index ID")?;
            let path = ManagedPath::new(format!("shared/assets/indexes/{index_id}.json"))?;
            resolved_download(index.into_download(), path, ArtifactKind::Metadata, config)
        })
        .transpose()?;

    let logging = dto
        .logging
        .and_then(|logging| logging.client)
        .map(|client| {
            bounded_string(&client.argument, "logging argument")?;
            let file_id = bounded_owned(client.file.id, "logging file ID")?;
            Ok::<_, GrapheneError>(ResolvedLogging {
                artifact: resolved_download(
                    client.file.download,
                    ManagedPath::new(format!("shared/metadata/logging/{file_id}"))?,
                    ArtifactKind::Metadata,
                    config,
                )?,
                argument: client.argument,
            })
        })
        .transpose()?;

    let java_requirement = dto
        .java_version
        .map(|java| {
            if java.major_version == 0 || java.major_version > 10_000 {
                return Err(mc_error(
                    ErrorCode::MinecraftMetadataInvalid,
                    "Java requirement major version is invalid",
                ));
            }
            if let Some(component) = &java.component {
                bounded_string(component, "Java component hint")?;
            }
            Ok::<_, GrapheneError>(MinecraftJavaRequirement {
                major_version: java.major_version,
                component_hint: java.component,
            })
        })
        .transpose()?;

    let assets_id = dto
        .assets
        .map(|value| bounded_owned(value, "assets ID"))
        .transpose()?;
    let inherits_from = dto.inherits_from.map(MinecraftVersionId::new).transpose()?;
    let main_class = dto
        .main_class
        .map(|value| bounded_owned(value, "main class"))
        .transpose()?;
    let legacy_minecraft_arguments = dto
        .minecraft_arguments
        .map(|value| bounded_owned(value, "legacy Minecraft arguments"))
        .transpose()?;

    Ok(MinecraftVersionMetadata {
        id,
        version_type: MinecraftVersionType::from_provider(&dto.version_type),
        inherits_from,
        main_class,
        client,
        jvm_args,
        game_args,
        legacy_minecraft_arguments,
        libraries,
        asset_index,
        assets_id,
        logging,
        java_requirement,
        release_time: dto.release_time,
        compliance_level: dto.compliance_level,
    })
}

pub(super) fn normalize_asset_index(
    index_id: &str,
    index: ResolvedArtifact,
    bytes: &[u8],
    base: &str,
    config: &MojangProviderConfig,
) -> Result<ResolvedAssets> {
    let dto: AssetIndexDto = serde_json::from_slice(bytes).map_err(|source| {
        mc_error(
            ErrorCode::MinecraftAssetIndexInvalid,
            "asset index JSON is invalid",
        )
        .with_source(source)
    })?;

    if dto.objects.len() > MAX_ASSET_OBJECTS {
        return Err(mc_error(
            ErrorCode::MinecraftAssetIndexInvalid,
            "asset index contains too many objects",
        ));
    }

    let mut objects = Vec::with_capacity(dto.objects.len());
    for (logical_name, object) in dto.objects {
        bounded_string(&logical_name, "asset logical name")?;

        let digest = parse_sha1_for(
            &object.hash,
            ErrorCode::MinecraftAssetIndexInvalid,
            "asset index contains an invalid SHA-1 digest",
        )?;

        let hash = digest.to_string();
        let prefix = &hash[..2];
        let url = format!("{}/{prefix}/{hash}", base.trim_end_matches('/'));
        let artifact = artifact_for(
            ArtifactKind::Binary,
            url,
            ArtifactIntegrity::none().with_sha1(digest),
            Some(object.size),
            config.allow_http,
        )?;

        objects.push(ResolvedAssetObject {
            logical_name,
            hash: hash.clone(),
            size: object.size,
            artifact: ResolvedArtifact {
                artifact,
                relative_path: ManagedPath::new(format!("shared/assets/objects/{prefix}/{hash}"))?,
            },
        });
    }

    Ok(ResolvedAssets {
        index_id: bounded_owned(index_id.to_owned(), "asset index ID")?,
        index,
        objects,
        virtual_layout: dto.virtual_layout,
        map_to_resources: dto.map_to_resources,
    })
}

fn normalize_arguments(values: Vec<ArgumentDto>) -> Result<Vec<Argument>> {
    values
        .into_iter()
        .map(|value| match value {
            ArgumentDto::Literal(value) => {
                bounded_string(&value, "launch argument")?;
                Ok(Argument::Literal(value))
            }
            ArgumentDto::Conditional { rules, value } => {
                let values = value.into_vec();
                if values.len() > 128 {
                    return Err(mc_error(
                        ErrorCode::MinecraftArgumentInvalid,
                        "conditional argument contains too many values",
                    ));
                }

                for value in &values {
                    bounded_string(value, "launch argument")?;
                }
                Ok(Argument::Conditional {
                    rules: normalize_rules(rules)?,
                    values,
                })
            }
        })
        .collect()
}

fn normalize_rules(values: Vec<RuleDto>) -> Result<Vec<Rule>> {
    values
        .into_iter()
        .map(|rule| {
            let action = match rule.action.as_str() {
                "allow" => RuleAction::Allow,
                "disallow" => RuleAction::Disallow,
                _ => {
                    return Err(mc_error(
                        ErrorCode::MinecraftRuleInvalid,
                        "rule action is unsupported",
                    ));
                }
            };

            let os = rule
                .os
                .map(|os| {
                    if let Some(pattern) = &os.version {
                        bounded_string(pattern, "OS version pattern")?;
                    }
                    Ok::<_, GrapheneError>(OsRule {
                        name: os.name.map(|name| normalize_os(&name)),
                        architecture: os.arch.map(|arch| normalize_arch(&arch)),
                        version_pattern: os.version,
                    })
                })
                .transpose()?;

            Ok(Rule {
                action,
                os,
                features: rule.features,
            })
        })
        .collect()
}

fn normalize_os(value: &str) -> MinecraftOs {
    match value.to_ascii_lowercase().as_str() {
        "windows" => MinecraftOs::Windows,
        "linux" => MinecraftOs::Linux,
        "osx" | "macos" => MinecraftOs::Osx,
        other => MinecraftOs::Other(other.to_owned()),
    }
}

fn normalize_arch(value: &str) -> MinecraftArch {
    match value.to_ascii_lowercase().as_str() {
        "x86" | "i386" | "i686" => MinecraftArch::X86,
        "x86_64" | "amd64" => MinecraftArch::X86_64,
        "aarch64" | "arm64" => MinecraftArch::AArch64,
        other => MinecraftArch::Other(other.to_owned()),
    }
}

#[cfg(test)]
mod tests;
