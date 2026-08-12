use super::{config::NeoForgeProviderConfig, dto::VersionsDto};
use crate::loader::{
    LoaderProvider, LoaderProviderFuture,
    common::{MAX_LOADER_VERSIONS, artifact, join_base, loader_error},
    forge_family::{ForgeFamily, normalize_verified_installer},
};
use graphene_core::{ArtifactIntegrity, ErrorCode, OperationController, Result};
use graphene_minecraft::{
    ComponentConflict, ComponentDescriptor, ComponentKind, ComponentProvenance,
    ComponentRequirement, ComponentUid, ComponentVersion, LoaderKind, LoaderProviderCapabilities,
    LoaderSupport, LoaderVersion, LoaderVersionSummary, ManagedPath, MinecraftVersionId,
    MinecraftVersionPatch, ResolvedArtifact, ResolvedComponent, ResolvedLoader,
};
use graphene_network::NetworkClient;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct NeoForgeProvider {
    network: NetworkClient,
    config: NeoForgeProviderConfig,
}

impl NeoForgeProvider {
    pub fn new(network: NetworkClient, config: NeoForgeProviderConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { network, config })
    }

    async fn all_versions(&self, operation: &OperationController) -> Result<Vec<String>> {
        let body = self
            .network
            .get_bytes_bounded(
                &self.config.versions_url,
                self.config.max_metadata_bytes,
                self.config.allow_http,
                operation,
            )
            .await?;

        let dto: VersionsDto = serde_json::from_slice(&body).map_err(|source| {
            loader_error(
                ErrorCode::LoaderMetadataInvalid,
                "NeoForge Maven version metadata is invalid JSON",
            )
            .with_source(source)
        })?;

        if dto.versions.len() > MAX_LOADER_VERSIONS {
            return Err(loader_error(
                ErrorCode::LoaderMetadataInvalid,
                "NeoForge version metadata exceeds the configured bound",
            ));
        }

        Ok(dto.versions)
    }

    async fn versions(
        &self,
        minecraft: &MinecraftVersionId,
        operation: &OperationController,
    ) -> Result<Vec<LoaderVersionSummary>> {
        let mut output = Vec::new();
        for value in self.all_versions(operation).await? {
            if !candidate_for_minecraft(&value, minecraft.as_str()) {
                continue;
            }

            let lower = value.to_ascii_lowercase();
            output.push(LoaderVersionSummary {
                kind: LoaderKind::NeoForge,
                version: LoaderVersion::new(value)?,
                minecraft: minecraft.clone(),
                stable: Some(
                    !lower.contains("beta") && !lower.contains("alpha") && !lower.contains("rc"),
                ),
                recommended: None,
                release_time: None,
                support: LoaderSupport::MetadataOnly {
                    reason: "base compatibility is finalized from the verified installer profile"
                        .to_owned(),
                },
            });
        }

        output.reverse();
        Ok(output)
    }

    async fn exact(
        &self,
        minecraft: &MinecraftVersionId,
        version: &LoaderVersion,
        operation: &OperationController,
    ) -> Result<ResolvedLoader> {
        let all = self.all_versions(operation).await?;

        if !all.iter().any(|candidate| candidate == version.as_str())
            || !candidate_for_minecraft(version.as_str(), minecraft.as_str())
        {
            return Err(loader_error(
                ErrorCode::LoaderVersionNotFound,
                "NeoForge release was not found for the requested Minecraft version",
            )
            .with_context("loader", "neoforge")
            .with_context("loader_version", version.to_string())
            .with_context("minecraft_version", minecraft.to_string()));
        }

        let repository_path = format!(
            "net/neoforged/neoforge/{0}/neoforge-{0}-installer.jar",
            version.as_str()
        );
        let url = join_base(&self.config.maven_base, &repository_path);
        let sha256 = self.checksum(&format!("{url}.sha256"), operation).await?;
        let integrity =
            ArtifactIntegrity::none().with_sha256(sha256.parse().map_err(|source| {
                loader_error(
                    ErrorCode::LoaderMetadataInvalid,
                    "NeoForge installer SHA-256 is invalid",
                )
                .with_source(source)
            })?);
        let installer = ResolvedArtifact {
            artifact: artifact(url, integrity, None, self.config.allow_http)?,
            relative_path: ManagedPath::new(format!(
                "shared/loader-installers/neoforge/{}/installer.jar",
                version.as_str()
            ))?,
        };

        loader_shell(minecraft, version, installer)
    }

    async fn checksum(&self, url: &str, operation: &OperationController) -> Result<String> {
        let body = self
            .network
            .get_bytes_bounded(url, 256, self.config.allow_http, operation)
            .await
            .map_err(|source| {
                loader_error(
                    ErrorCode::LoaderArtifactUnverifiable,
                    "NeoForge installer checksum sidecar is unavailable",
                )
                .with_source(source)
            })?;

        let value = std::str::from_utf8(&body)
            .map_err(|source| {
                loader_error(
                    ErrorCode::LoaderMetadataInvalid,
                    "NeoForge installer checksum sidecar is not UTF-8",
                )
                .with_source(source)
            })?
            .split_whitespace()
            .next()
            .unwrap_or_default();

        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(loader_error(
                ErrorCode::LoaderMetadataInvalid,
                "NeoForge installer checksum sidecar is invalid",
            ));
        }

        Ok(value.to_ascii_lowercase())
    }
}

impl LoaderProvider for NeoForgeProvider {
    fn kind(&self) -> LoaderKind {
        LoaderKind::NeoForge
    }

    fn capabilities(&self) -> LoaderProviderCapabilities {
        LoaderProviderCapabilities::VERSION_LIST
            .union(LoaderProviderCapabilities::EXACT_RESOLUTION)
            .union(LoaderProviderCapabilities::INSTALLER_ARCHIVE)
            .union(LoaderProviderCapabilities::PROCESSOR_RECIPE)
            .union(LoaderProviderCapabilities::CHECKSUM_SIDECARS)
    }

    fn list_versions<'a>(
        &'a self,
        minecraft: &'a MinecraftVersionId,
        operation: &'a OperationController,
    ) -> LoaderProviderFuture<'a, Vec<LoaderVersionSummary>> {
        Box::pin(self.versions(minecraft, operation))
    }

    fn resolve_exact<'a>(
        &'a self,
        minecraft: &'a MinecraftVersionId,
        version: &'a LoaderVersion,
        operation: &'a OperationController,
    ) -> LoaderProviderFuture<'a, ResolvedLoader> {
        Box::pin(self.exact(minecraft, version, operation))
    }

    fn normalize_verified_installer<'a>(
        &'a self,
        resolved: ResolvedLoader,
        verified_installer: PathBuf,
        operation: &'a OperationController,
    ) -> LoaderProviderFuture<'a, ResolvedLoader> {
        Box::pin(async move {
            normalize_verified_installer(
                resolved,
                &verified_installer,
                operation,
                ForgeFamily::NeoForge,
                &self.config.maven_base,
                self.config.allow_http,
            )
        })
    }
}

fn candidate_for_minecraft(loader: &str, minecraft: &str) -> bool {
    if let Some(rest) = minecraft.strip_prefix("1.") {
        return loader.starts_with(&format!("{rest}."));
    }

    loader == minecraft || loader.starts_with(&format!("{minecraft}."))
}

fn loader_shell(
    minecraft: &MinecraftVersionId,
    version: &LoaderVersion,
    installer: ResolvedArtifact,
) -> Result<ResolvedLoader> {
    let kind = LoaderKind::NeoForge;
    let uid = ComponentUid::new(kind.component_uid())?;
    let component_version = ComponentVersion::new(version.as_str())?;
    let provenance = ComponentProvenance::new(
        "neoforge-maven",
        Some("verified-installer-pending".to_owned()),
    )?;
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
                uid: ComponentUid::new(LoaderKind::Fabric.component_uid())?,
            },
            ComponentConflict {
                uid: ComponentUid::new(LoaderKind::Forge.component_uid())?,
            },
        ],
    };

    Ok(ResolvedLoader {
        kind,
        version: version.clone(),
        minecraft: minecraft.clone(),
        component: descriptor,
        patch: MinecraftVersionPatch::empty(resolved_component.clone()),
        preparation: graphene_minecraft::ComponentPreparationRecipe {
            component: Some(resolved_component.clone()),
            installer: Some(installer),
            ..Default::default()
        },
        support: LoaderSupport::MetadataOnly {
            reason: "verified installer normalization is required before installation".to_owned(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_prefilter_is_provider_specific_and_not_domain_semver() {
        assert!(candidate_for_minecraft("21.1.200", "1.21.1"));
        assert!(candidate_for_minecraft("20.4.250", "1.20.4"));
        assert!(candidate_for_minecraft("26.1.2.3", "26.1.2"));
        assert!(candidate_for_minecraft("26.2.0", "26.2.0"));
        assert!(!candidate_for_minecraft("21.0.200", "1.21.1"));
    }
}
