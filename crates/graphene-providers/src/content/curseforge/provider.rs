use crate::content::curseforge::{
    config::CurseForgeProviderConfig,
    dto::{
        CurseForgeFilesResponseDto, CurseForgeFingerprintsResponseDto, CurseForgeSearchResponseDto,
        CurseForgeSingleFileResponseDto, CurseForgeSingleModResponseDto,
    },
    normalize::{
        normalize_file, normalize_files_response, normalize_project, normalize_search_page,
        provider_id,
    },
};
use graphene_content::{
    ContentFileMatch, ContentFileRef, ContentProject, ContentProjectRef, ContentProvider,
    ContentProviderCapabilities, ContentProviderFuture, ContentProviderId, ContentSearchQuery,
    ContentVersion, ContentVersionFilter, ContentVersionRef, ExactMatchStatus, FileMatchRequest,
    InstanceContentContext, ReleaseChannelPolicy, compatibility::evaluate_compatibility,
};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, OperationController, Result};
use graphene_minecraft::LoaderKind;
use graphene_network::NetworkClient;

const MAX_SEARCH_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_MOD_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_FILES_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_FILE_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_MATCH_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

fn percent_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            use std::fmt::Write as _;
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

fn loader_type_code(loader: LoaderKind) -> &'static str {
    match loader {
        LoaderKind::Fabric => "4",
        LoaderKind::Forge => "1",
        LoaderKind::NeoForge => "6",
        _ => "0",
    }
}

/// CurseForge implementation of `ContentProvider`.
#[derive(Debug, Clone)]
pub struct CurseForgeContentProvider {
    network: NetworkClient,
    config: CurseForgeProviderConfig,
    id: ContentProviderId,
}

impl CurseForgeContentProvider {
    pub fn new(network: NetworkClient, config: CurseForgeProviderConfig) -> Result<Self> {
        config.validate()?;
        let id = provider_id();
        Ok(Self {
            network,
            config,
            id,
        })
    }

    pub(crate) fn check_key(&self) -> Result<&str> {
        self.config
            .api_key()
            .map(|k| k.expose_secret())
            .ok_or_else(|| {
                GrapheneError::new(
                    ErrorCode::ContentProviderUnavailable,
                    ErrorKind::Content,
                    "CurseForge provider is unavailable: no API key configured",
                )
            })
    }
}

impl ContentProvider for CurseForgeContentProvider {
    fn id(&self) -> &ContentProviderId {
        &self.id
    }

    fn capabilities(&self) -> ContentProviderCapabilities {
        ContentProviderCapabilities::curseforge()
    }

    fn search<'a>(
        &'a self,
        query: &'a ContentSearchQuery,
        operation: &'a OperationController,
    ) -> ContentProviderFuture<'a, graphene_content::ContentSearchPage> {
        Box::pin(async move {
            let key = self.check_key()?;
            let mut url = format!(
                "{}/mods/search?gameId={}&searchFilter={}&index={}&pageSize={}",
                self.config.base_url().trim_end_matches('/'),
                self.config.game_id(),
                percent_encode(&query.query),
                query.offset,
                query.limit.min(50)
            );

            if let Some(mv) = &query.minecraft_version {
                url.push_str(&format!("&gameVersion={}", percent_encode(mv)));
            }
            if let Some(l) = &query.loader {
                url.push_str(&format!("&modLoaderType={}", loader_type_code(*l)));
            }

            let headers = [("x-api-key", key)];
            let response = self
                .network
                .get_with_headers_bounded(
                    &url,
                    &headers,
                    MAX_SEARCH_RESPONSE_BYTES,
                    self.config.allow_http(),
                    operation,
                )
                .await?;

            let dto: CurseForgeSearchResponseDto =
                serde_json::from_slice(&response.body).map_err(|source| {
                    GrapheneError::new(
                        ErrorCode::NetworkStatusError,
                        ErrorKind::Network,
                        "failed to parse CurseForge search response",
                    )
                    .with_source(source)
                })?;

            normalize_search_page(dto)
        })
    }

    fn project<'a>(
        &'a self,
        project_ref: &'a ContentProjectRef,
        operation: &'a OperationController,
    ) -> ContentProviderFuture<'a, ContentProject> {
        Box::pin(async move {
            let key = self.check_key()?;
            let url = format!(
                "{}/mods/{}",
                self.config.base_url().trim_end_matches('/'),
                project_ref.project_id
            );

            let headers = [("x-api-key", key)];
            let response = self
                .network
                .get_with_headers_bounded(
                    &url,
                    &headers,
                    MAX_MOD_RESPONSE_BYTES,
                    self.config.allow_http(),
                    operation,
                )
                .await?;

            if response.status == 404 {
                return Err(GrapheneError::new(
                    ErrorCode::ContentProjectNotFound,
                    ErrorKind::Content,
                    format!("project {} not found on CurseForge", project_ref.project_id),
                ));
            }

            let dto: CurseForgeSingleModResponseDto = serde_json::from_slice(&response.body)
                .map_err(|source| {
                    GrapheneError::new(
                        ErrorCode::NetworkStatusError,
                        ErrorKind::Network,
                        "failed to parse CurseForge mod response",
                    )
                    .with_source(source)
                })?;

            normalize_project(dto.data)
        })
    }

    fn list_versions<'a>(
        &'a self,
        project_ref: &'a ContentProjectRef,
        filters: &'a ContentVersionFilter,
        operation: &'a OperationController,
    ) -> ContentProviderFuture<'a, Vec<ContentVersion>> {
        Box::pin(async move {
            let key = self.check_key()?;
            let mut url = format!(
                "{}/mods/{}/files?",
                self.config.base_url().trim_end_matches('/'),
                project_ref.project_id
            );

            if let Some(mv) = &filters.minecraft_version {
                url.push_str(&format!("gameVersion={}&", percent_encode(mv)));
            }
            if let Some(l) = &filters.loader {
                url.push_str(&format!("modLoaderType={}&", loader_type_code(*l)));
            }

            let headers = [("x-api-key", key)];
            let response = self
                .network
                .get_with_headers_bounded(
                    &url,
                    &headers,
                    MAX_FILES_RESPONSE_BYTES,
                    self.config.allow_http(),
                    operation,
                )
                .await?;

            if response.status == 404 {
                return Err(GrapheneError::new(
                    ErrorCode::ContentProjectNotFound,
                    ErrorKind::Content,
                    format!("project {} not found on CurseForge", project_ref.project_id),
                ));
            }

            let dto: CurseForgeFilesResponseDto =
                serde_json::from_slice(&response.body).map_err(|source| {
                    GrapheneError::new(
                        ErrorCode::NetworkStatusError,
                        ErrorKind::Network,
                        "failed to parse CurseForge files response",
                    )
                    .with_source(source)
                })?;

            normalize_files_response(dto)
        })
    }

    fn get_version<'a>(
        &'a self,
        version_ref: &'a ContentVersionRef,
        operation: &'a OperationController,
    ) -> ContentProviderFuture<'a, ContentVersion> {
        Box::pin(async move {
            let key = self.check_key()?;
            let url = format!(
                "{}/mods/{}/files/{}",
                self.config.base_url().trim_end_matches('/'),
                version_ref.project_id,
                version_ref.version_id
            );

            let headers = [("x-api-key", key)];
            let response = self
                .network
                .get_with_headers_bounded(
                    &url,
                    &headers,
                    MAX_FILE_RESPONSE_BYTES,
                    self.config.allow_http(),
                    operation,
                )
                .await?;

            if response.status == 404 {
                return Err(GrapheneError::new(
                    ErrorCode::ContentVersionNotFound,
                    ErrorKind::Content,
                    format!("file {} not found on CurseForge", version_ref.version_id),
                ));
            }

            let dto: CurseForgeSingleFileResponseDto = serde_json::from_slice(&response.body)
                .map_err(|source| {
                    GrapheneError::new(
                        ErrorCode::NetworkStatusError,
                        ErrorKind::Network,
                        "failed to parse CurseForge file response",
                    )
                    .with_source(source)
                })?;

            let (v, _) = normalize_file(dto.data, &self.id)?;
            Ok(v)
        })
    }

    fn match_files<'a>(
        &'a self,
        request: &'a FileMatchRequest,
        operation: &'a OperationController,
    ) -> ContentProviderFuture<'a, Vec<ContentFileMatch>> {
        Box::pin(async move {
            let key = match self.check_key() {
                Ok(k) => k,
                Err(_) => {
                    return Ok(request
                        .items
                        .iter()
                        .map(|it| ContentFileMatch {
                            item: it.clone(),
                            status: ExactMatchStatus::Unmatched,
                        })
                        .collect());
                }
            };

            let mut results = Vec::with_capacity(request.items.len());
            let fingerprints: Vec<u32> = request
                .items
                .iter()
                .filter_map(|it| it.murmur2_fingerprint)
                .collect();

            if fingerprints.is_empty() {
                for it in &request.items {
                    results.push(ContentFileMatch {
                        item: it.clone(),
                        status: ExactMatchStatus::Unmatched,
                    });
                }
                return Ok(results);
            }

            let body_json = serde_json::json!({
                "fingerprints": fingerprints,
            });
            let body_bytes = serde_json::to_vec(&body_json).unwrap_or_default();

            let url = format!(
                "{}/fingerprints",
                self.config.base_url().trim_end_matches('/')
            );
            let headers = [("x-api-key", key)];
            let response = self
                .network
                .post_json_with_headers_bounded(
                    &url,
                    &body_bytes,
                    &headers,
                    MAX_MATCH_RESPONSE_BYTES,
                    self.config.allow_http(),
                    operation,
                )
                .await?;

            let match_dto: CurseForgeFingerprintsResponseDto =
                serde_json::from_slice(&response.body).map_err(|source| {
                    GrapheneError::new(
                        ErrorCode::NetworkStatusError,
                        ErrorKind::Network,
                        "failed to parse CurseForge fingerprints response",
                    )
                    .with_source(source)
                })?;

            let exact_matches = match_dto.data.exact_matches.unwrap_or_default();

            for item in &request.items {
                let matched = item.murmur2_fingerprint.and_then(|fp| {
                    exact_matches
                        .iter()
                        .find(|em| em.file.package_fingerprint == Some(fp))
                });

                if let Some(m) = matched {
                    let (normalized_ver, normalized_file) =
                        normalize_file(m.file.clone(), &self.id)?;
                    if normalized_file.size == item.size {
                        results.push(ContentFileMatch {
                            item: item.clone(),
                            status: ExactMatchStatus::Matched {
                                version: Box::new(normalized_ver),
                                file: Box::new(normalized_file),
                            },
                        });
                        continue;
                    }
                }

                results.push(ContentFileMatch {
                    item: item.clone(),
                    status: ExactMatchStatus::Unmatched,
                });
            }

            Ok(results)
        })
    }

    fn find_update<'a>(
        &'a self,
        current_file: &'a ContentFileRef,
        context: &'a InstanceContentContext,
        policy: ReleaseChannelPolicy,
        operation: &'a OperationController,
    ) -> ContentProviderFuture<'a, Option<ContentVersion>> {
        Box::pin(async move {
            let filter = ContentVersionFilter {
                minecraft_version: Some(context.minecraft_version.clone()),
                loader: context.loader,
            };

            let versions = self
                .list_versions(&current_file.project_ref(), &filter, operation)
                .await?;

            let mut candidates: Vec<ContentVersion> = versions
                .into_iter()
                .filter(|v| {
                    evaluate_compatibility(v, context, policy).is_compatible()
                        && v.primary_file().is_some()
                })
                .collect();

            if candidates.is_empty() {
                return Ok(None);
            }

            candidates.sort_by(|a, b| {
                a.release_channel
                    .cmp(&b.release_channel)
                    .then_with(|| b.date_published.cmp(&a.date_published))
            });

            Ok(candidates.into_iter().next())
        })
    }
}
