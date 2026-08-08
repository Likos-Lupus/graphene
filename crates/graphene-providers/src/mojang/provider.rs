use super::{
    artifact::{deduplicate_resolved_artifacts, metadata_version_path},
    config::MojangProviderConfig,
    error::{mc_error, provider_network_error},
    normalize::{normalize_asset_index, normalize_manifest, normalize_version_metadata},
};
use graphene_core::{Artifact, ErrorCode, OperationController, OperationHandle, Result};
use graphene_minecraft::{
    InheritanceTracker, MinecraftVersionId, MinecraftVersionMetadata, ResolvedArtifact,
    ResolvedMinecraft, RuleContext, VersionManifest, merge_metadata, resolve_minecraft,
};
use graphene_network::NetworkClient;
use std::{future::Future, pin::Pin};

const MAX_MANIFEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_VERSION_BYTES: usize = 8 * 1024 * 1024;
const MAX_ASSET_INDEX_BYTES: usize = 32 * 1024 * 1024;

/// Provider-side verified-metadata acquisition port. The service adapter delegates to Phase 0's
/// artifact service so version and asset-index documents share the verified cache pipeline.
pub trait MetadataArtifactAcquirer: Send + Sync {
    fn acquire<'a>(
        &'a self,
        artifact: Artifact,
        parent: OperationHandle,
        max_bytes: usize,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>>;
}

/// Provider-neutral resolution result including metadata artifacts the installer may materialize.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMinecraftBundle {
    pub minecraft: ResolvedMinecraft,
    pub metadata_artifacts: Vec<ResolvedArtifact>,
}

#[derive(Debug, Clone)]
pub struct MojangProvider {
    network: NetworkClient,
    config: MojangProviderConfig,
}

impl MojangProvider {
    pub fn new(network: NetworkClient, config: MojangProviderConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { network, config })
    }

    pub async fn manifest(&self, operation: &OperationController) -> Result<VersionManifest> {
        let bytes = self
            .network
            .get_bytes_bounded(
                &self.config.manifest_url,
                MAX_MANIFEST_BYTES,
                self.config.allow_http,
                operation,
            )
            .await
            .map_err(provider_network_error)?;
        normalize_manifest(&bytes, &self.config)
    }

    pub async fn resolve(
        &self,
        requested: &MinecraftVersionId,
        context: &RuleContext,
        acquirer: &dyn MetadataArtifactAcquirer,
        operation: &OperationController,
    ) -> Result<ResolvedMinecraftBundle> {
        let manifest = self.manifest(operation).await?;
        manifest.select(requested)?;
        let mut inheritance = InheritanceTracker::new();
        let mut metadata_artifacts = Vec::new();
        let metadata = self
            .resolve_metadata(
                requested,
                &manifest,
                acquirer,
                operation.handle(),
                &mut inheritance,
                &mut metadata_artifacts,
            )
            .await?;
        let asset_index = metadata.asset_index.clone().ok_or_else(|| {
            mc_error(
                ErrorCode::MinecraftMetadataUnsupported,
                "Tier A metadata does not declare an asset index",
            )
        })?;
        let assets_id = metadata.assets_id.clone().ok_or_else(|| {
            mc_error(
                ErrorCode::MinecraftMetadataUnsupported,
                "Tier A metadata does not declare an asset ID",
            )
        })?;
        let asset_bytes = acquirer
            .acquire(
                asset_index.artifact.clone(),
                operation.handle(),
                MAX_ASSET_INDEX_BYTES,
            )
            .await?;
        let assets = normalize_asset_index(
            &assets_id,
            asset_index.clone(),
            &asset_bytes,
            &self.config.asset_object_base,
            &self.config,
        )?;

        metadata_artifacts.push(asset_index);
        deduplicate_resolved_artifacts(&mut metadata_artifacts);

        let minecraft = resolve_minecraft(metadata, assets, context)?;
        Ok(ResolvedMinecraftBundle {
            minecraft,
            metadata_artifacts,
        })
    }

    fn resolve_metadata<'a>(
        &'a self,
        requested: &'a MinecraftVersionId,
        manifest: &'a VersionManifest,
        acquirer: &'a dyn MetadataArtifactAcquirer,
        parent: OperationHandle,
        inheritance: &'a mut InheritanceTracker,
        metadata_artifacts: &'a mut Vec<ResolvedArtifact>,
    ) -> Pin<Box<dyn Future<Output = Result<MinecraftVersionMetadata>> + Send + 'a>> {
        Box::pin(async move {
            inheritance.enter(requested)?;
            let result = async {
                let summary = manifest.select(requested)?;
                if !summary.metadata.integrity.is_verifiable() {
                    return Err(mc_error(
                        ErrorCode::MinecraftMetadataUnsupported,
                        "version metadata lacks a trustworthy digest",
                    )
                    .with_context("version_id", requested.to_string()));
                }

                let bytes = acquirer
                    .acquire(summary.metadata.clone(), parent.clone(), MAX_VERSION_BYTES)
                    .await?;
                let mut child = normalize_version_metadata(&bytes, &self.config)?;
                if &child.id != requested {
                    return Err(mc_error(
                        ErrorCode::MinecraftMetadataInvalid,
                        "version metadata ID does not match the requested version",
                    )
                    .with_context("version_id", requested.to_string()));
                }

                metadata_artifacts.push(ResolvedArtifact {
                    artifact: summary.metadata.clone(),
                    relative_path: metadata_version_path(requested)?,
                });

                let result = if let Some(parent_id) = child.inherits_from.clone() {
                    let parent_metadata = self
                        .resolve_metadata(
                            &parent_id,
                            manifest,
                            acquirer,
                            parent.clone(),
                            inheritance,
                            metadata_artifacts,
                        )
                        .await?;
                    child.inherits_from = None;
                    merge_metadata(parent_metadata, child)?
                } else {
                    child
                };

                Ok(result)
            }
            .await;
            inheritance.leave(requested);
            result
        })
    }
}
