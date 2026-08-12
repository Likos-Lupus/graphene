use super::{
    config::FabricProviderConfig,
    dto::{LoaderVersionEnvelope, ProfileDto},
    normalize::normalize_profile,
};
use crate::loader::{
    LoaderProvider, LoaderProviderFuture,
    common::{
        MAX_LOADER_VERSIONS, join_base, loader_error, percent_encode_segment, validate_endpoint,
    },
};
use graphene_core::{ErrorCode, OperationController, Result};
use graphene_minecraft::{
    LoaderKind, LoaderProviderCapabilities, LoaderSupport, LoaderVersion, LoaderVersionSummary,
    MavenCoordinate, MinecraftVersionId, ResolvedLoader,
};
use graphene_network::NetworkClient;

#[derive(Debug, Clone)]
pub struct FabricProvider {
    network: NetworkClient,
    config: FabricProviderConfig,
}

impl FabricProvider {
    pub fn new(network: NetworkClient, config: FabricProviderConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { network, config })
    }

    async fn versions(
        &self,
        minecraft: &MinecraftVersionId,
        operation: &OperationController,
    ) -> Result<Vec<LoaderVersionSummary>> {
        let url = join_base(
            &self.config.meta_base,
            &format!(
                "v2/versions/loader/{}",
                percent_encode_segment(minecraft.as_str())
            ),
        );
        let body = self
            .network
            .get_bytes_bounded(
                &url,
                self.config.max_metadata_bytes,
                self.config.allow_http,
                operation,
            )
            .await?;
        let dto: Vec<LoaderVersionEnvelope> = serde_json::from_slice(&body).map_err(|source| {
            loader_error(
                ErrorCode::LoaderMetadataInvalid,
                "Fabric loader version metadata is invalid JSON",
            )
            .with_source(source)
        })?;

        if dto.len() > MAX_LOADER_VERSIONS {
            return Err(loader_error(
                ErrorCode::LoaderMetadataInvalid,
                "Fabric loader version list exceeds the bound",
            ));
        }

        dto.into_iter()
            .map(|entry| {
                Ok(LoaderVersionSummary {
                    kind: LoaderKind::Fabric,
                    version: LoaderVersion::new(entry.loader.version)?,
                    minecraft: minecraft.clone(),
                    stable: Some(entry.loader.stable),
                    recommended: None,
                    release_time: None,
                    support: LoaderSupport::Supported,
                })
            })
            .collect()
    }

    async fn fill_missing_hashes(
        &self,
        profile: &mut ProfileDto,
        operation: &OperationController,
    ) -> Result<()> {
        for library in &mut profile.libraries {
            if library.sha1.is_some() || library.sha256.is_some() {
                continue;
            }

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
                .as_deref()
                .unwrap_or(&self.config.default_maven_base);

            validate_endpoint(base, self.config.allow_http)?;

            let artifact_url = join_base(base, path.as_str());
            let checksum_url = format!("{artifact_url}.sha1");
            let body = self
                .network
                .get_bytes_bounded(&checksum_url, 256, self.config.allow_http, operation)
                .await
                .map_err(|source| {
                    loader_error(
                        ErrorCode::LoaderArtifactUnverifiable,
                        "Fabric library has no inline hash and its checksum sidecar is unavailable",
                    )
                    .with_source(source)
                })?;
            let value = std::str::from_utf8(&body)
                .map_err(|source| {
                    loader_error(
                        ErrorCode::LoaderMetadataInvalid,
                        "Fabric Maven checksum sidecar is not UTF-8",
                    )
                    .with_source(source)
                })?
                .trim();

            if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(loader_error(
                    ErrorCode::LoaderMetadataInvalid,
                    "Fabric Maven checksum sidecar is invalid",
                ));
            }

            library.sha1 = Some(value.to_owned());
        }

        Ok(())
    }

    async fn exact(
        &self,
        minecraft: &MinecraftVersionId,
        version: &LoaderVersion,
        operation: &OperationController,
    ) -> Result<ResolvedLoader> {
        let available = self.versions(minecraft, operation).await?;
        if !available.iter().any(|entry| &entry.version == version) {
            return Err(loader_error(
                ErrorCode::LoaderVersionNotFound,
                "Fabric loader version was not found for the requested Minecraft version",
            )
            .with_context("loader", LoaderKind::Fabric.to_string())
            .with_context("loader_version", version.to_string())
            .with_context("minecraft_version", minecraft.to_string()));
        }

        let url = join_base(
            &self.config.meta_base,
            &format!(
                "v2/versions/loader/{}/{}/profile/json",
                percent_encode_segment(minecraft.as_str()),
                percent_encode_segment(version.as_str())
            ),
        );

        let body = self
            .network
            .get_bytes_bounded(
                &url,
                self.config.max_metadata_bytes,
                self.config.allow_http,
                operation,
            )
            .await?;

        let mut dto: ProfileDto = serde_json::from_slice(&body).map_err(|source| {
            loader_error(
                ErrorCode::LoaderProfileInvalid,
                "Fabric loader profile is invalid JSON",
            )
            .with_source(source)
        })?;

        self.fill_missing_hashes(&mut dto, operation).await?;
        normalize_profile(dto, minecraft, version, &self.config)
    }
}

impl LoaderProvider for FabricProvider {
    fn kind(&self) -> LoaderKind {
        LoaderKind::Fabric
    }

    fn capabilities(&self) -> LoaderProviderCapabilities {
        LoaderProviderCapabilities::VERSION_LIST
            .union(LoaderProviderCapabilities::EXACT_RESOLUTION)
            .union(LoaderProviderCapabilities::PROFILE_PATCH)
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
}
