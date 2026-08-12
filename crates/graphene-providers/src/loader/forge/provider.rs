use super::{config::ForgeProviderConfig, dto::PromotionsDto};
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
pub struct ForgeProvider {
    network: NetworkClient,
    config: ForgeProviderConfig,
}

impl ForgeProvider {
    pub fn new(network: NetworkClient, config: ForgeProviderConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { network, config })
    }

    async fn promotions(&self, operation: &OperationController) -> Result<PromotionsDto> {
        let body = self
            .network
            .get_bytes_bounded(
                &self.config.promotions_url,
                self.config.max_metadata_bytes,
                self.config.allow_http,
                operation,
            )
            .await?;
        serde_json::from_slice(&body).map_err(|source| {
            loader_error(
                ErrorCode::LoaderMetadataInvalid,
                "Forge promotions metadata is invalid JSON",
            )
            .with_source(source)
        })
    }

    async fn versions(
        &self,
        minecraft: &MinecraftVersionId,
        operation: &OperationController,
    ) -> Result<Vec<LoaderVersionSummary>> {
        let promotions = self.promotions(operation).await?;
        let recommended = promotions
            .promos
            .get(&format!("{}-recommended", minecraft.as_str()));
        let latest = promotions
            .promos
            .get(&format!("{}-latest", minecraft.as_str()));
        let mut values = Vec::<(String, bool)>::new();

        if let Some(value) = latest {
            values.push((value.clone(), recommended == Some(value)));
        }

        if let Some(value) = recommended
            && !values.iter().any(|(existing, _)| existing == value)
        {
            values.push((value.clone(), true));
        }

        if values.len() > MAX_LOADER_VERSIONS {
            return Err(loader_error(
                ErrorCode::LoaderMetadataInvalid,
                "Forge promotions metadata exceeds the configured version bound",
            ));
        }

        values
            .into_iter()
            .map(|(value, is_recommended)| {
                Ok(LoaderVersionSummary {
                    kind: LoaderKind::Forge,
                    version: LoaderVersion::new(value)?,
                    minecraft: minecraft.clone(),
                    stable: Some(true),
                    recommended: Some(is_recommended),
                    release_time: None,
                    support: LoaderSupport::MetadataOnly {
                        reason: "support is finalized from the verified installer schema"
                            .to_owned(),
                    },
                })
            })
            .collect()
    }

    async fn exact(
        &self,
        minecraft: &MinecraftVersionId,
        version: &LoaderVersion,
        operation: &OperationController,
    ) -> Result<ResolvedLoader> {
        let combined = format!("{}-{}", minecraft.as_str(), version.as_str());
        let repository_path =
            format!("net/minecraftforge/forge/{combined}/forge-{combined}-installer.jar");
        let url = join_base(&self.config.maven_base, &repository_path);
        let sha1 = self.checksum(&format!("{url}.sha1"), operation).await?;
        let integrity = ArtifactIntegrity::none().with_sha1(sha1.parse().map_err(|source| {
            loader_error(
                ErrorCode::LoaderMetadataInvalid,
                "Forge installer SHA-1 is invalid",
            )
            .with_source(source)
        })?);
        let installer = ResolvedArtifact {
            artifact: artifact(url, integrity, None, self.config.allow_http)?,
            relative_path: ManagedPath::new(format!(
                "shared/loader-installers/forge/{combined}/installer.jar"
            ))?,
        };

        loader_shell(
            LoaderKind::Forge,
            minecraft,
            version,
            installer,
            "forge-maven",
        )
    }

    async fn checksum(&self, url: &str, operation: &OperationController) -> Result<String> {
        let body = self
            .network
            .get_bytes_bounded(url, 256, self.config.allow_http, operation)
            .await
            .map_err(|source| {
                loader_error(
                    ErrorCode::LoaderArtifactUnverifiable,
                    "Forge installer checksum sidecar is unavailable",
                )
                .with_source(source)
            })?;

        let value = std::str::from_utf8(&body)
            .map_err(|source| {
                loader_error(
                    ErrorCode::LoaderMetadataInvalid,
                    "Forge installer checksum sidecar is not UTF-8",
                )
                .with_source(source)
            })?
            .split_whitespace()
            .next()
            .unwrap_or_default();

        if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(loader_error(
                ErrorCode::LoaderMetadataInvalid,
                "Forge installer checksum sidecar is invalid",
            ));
        }

        Ok(value.to_ascii_lowercase())
    }
}

impl LoaderProvider for ForgeProvider {
    fn kind(&self) -> LoaderKind {
        LoaderKind::Forge
    }

    fn capabilities(&self) -> LoaderProviderCapabilities {
        LoaderProviderCapabilities::VERSION_LIST
            .union(LoaderProviderCapabilities::EXACT_RESOLUTION)
            .union(LoaderProviderCapabilities::INSTALLER_ARCHIVE)
            .union(LoaderProviderCapabilities::PROCESSOR_RECIPE)
            .union(LoaderProviderCapabilities::RECOMMENDED_RELEASE)
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
                ForgeFamily::Forge,
                &self.config.maven_base,
                self.config.allow_http,
            )
        })
    }
}

fn loader_shell(
    kind: LoaderKind,
    minecraft: &MinecraftVersionId,
    version: &LoaderVersion,
    installer: ResolvedArtifact,
    provider: &str,
) -> Result<ResolvedLoader> {
    let uid = ComponentUid::new(kind.component_uid())?;
    let component_version = ComponentVersion::new(version.as_str())?;
    let provenance =
        ComponentProvenance::new(provider, Some("verified-installer-pending".to_owned()))?;
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
        conflicts: [LoaderKind::Fabric, LoaderKind::Forge, LoaderKind::NeoForge]
            .into_iter()
            .filter(|candidate| *candidate != kind)
            .map(|candidate| {
                Ok(ComponentConflict {
                    uid: ComponentUid::new(candidate.component_uid())?,
                })
            })
            .collect::<Result<Vec<_>>>()?,
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
