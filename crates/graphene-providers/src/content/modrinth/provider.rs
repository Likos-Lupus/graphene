use crate::content::modrinth::{
    config::ModrinthProviderConfig,
    dto::{
        ModrinthProjectDto, ModrinthSearchResponseDto, ModrinthVersionDto,
        ModrinthVersionFilesMatchDto,
    },
    normalize::{normalize_project, normalize_search_page, normalize_version, provider_id},
};
use graphene_content::{
    ContentFileMatch, ContentFileRef, ContentProject, ContentProjectRef, ContentProvider,
    ContentProviderCapabilities, ContentProviderFuture, ContentProviderId, ContentSearchQuery,
    ContentVersion, ContentVersionFilter, ContentVersionRef, ExactMatchStatus, FileMatchRequest,
    InstanceContentContext, ReleaseChannelPolicy, compatibility::evaluate_compatibility,
};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, OperationController, Result};
use graphene_network::NetworkClient;

const MAX_SEARCH_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_PROJECT_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_VERSIONS_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_VERSION_RESPONSE_BYTES: usize = 1024 * 1024;
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

/// Reference Modrinth implementation of `ContentProvider`.
#[derive(Debug, Clone)]
pub struct ModrinthContentProvider {
    network: NetworkClient,
    config: ModrinthProviderConfig,
    id: ContentProviderId,
}

impl ModrinthContentProvider {
    pub fn new(network: NetworkClient, config: ModrinthProviderConfig) -> Result<Self> {
        config.validate()?;
        let id = provider_id();
        Ok(Self {
            network,
            config,
            id,
        })
    }

    fn headers(&self) -> [(&'static str, &str); 1] {
        [("User-Agent", &self.config.user_agent)]
    }
}

impl ContentProvider for ModrinthContentProvider {
    fn id(&self) -> &ContentProviderId {
        &self.id
    }

    fn capabilities(&self) -> ContentProviderCapabilities {
        ContentProviderCapabilities::modrinth()
    }

    fn search<'a>(
        &'a self,
        query: &'a ContentSearchQuery,
        operation: &'a OperationController,
    ) -> ContentProviderFuture<'a, graphene_content::ContentSearchPage> {
        Box::pin(async move {
            let mut url = format!(
                "{}/search?query={}&offset={}&limit={}",
                self.config.base_url.trim_end_matches('/'),
                percent_encode(&query.query),
                query.offset,
                query.limit.min(100)
            );

            // Construct facets for minecraft version and loader
            let mut facets: Vec<Vec<String>> = Vec::new();
            facets.push(vec!["project_type:mod".to_string()]);

            if let Some(mv) = &query.minecraft_version {
                facets.push(vec![format!("versions:{mv}")]);
            }
            if let Some(l) = &query.loader {
                facets.push(vec![format!("categories:{}", l.to_string().to_lowercase())]);
            }

            if let Ok(facets_json) = serde_json::to_string(&facets) {
                url.push_str("&facets=");
                url.push_str(&percent_encode(&facets_json));
            }

            let response = self
                .network
                .get_with_headers_bounded(
                    &url,
                    &self.headers(),
                    MAX_SEARCH_RESPONSE_BYTES,
                    self.config.allow_http,
                    operation,
                )
                .await?;

            let dto: ModrinthSearchResponseDto =
                serde_json::from_slice(&response.body).map_err(|source| {
                    GrapheneError::new(
                        ErrorCode::NetworkStatusError,
                        ErrorKind::Network,
                        "failed to parse Modrinth search response",
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
            let url = format!(
                "{}/project/{}",
                self.config.base_url.trim_end_matches('/'),
                project_ref.project_id
            );

            let response = self
                .network
                .get_with_headers_bounded(
                    &url,
                    &self.headers(),
                    MAX_PROJECT_RESPONSE_BYTES,
                    self.config.allow_http,
                    operation,
                )
                .await?;

            if response.status == 404 {
                return Err(GrapheneError::new(
                    ErrorCode::ContentProjectNotFound,
                    ErrorKind::Content,
                    format!("project {} not found on Modrinth", project_ref.project_id),
                ));
            }

            let dto: ModrinthProjectDto =
                serde_json::from_slice(&response.body).map_err(|source| {
                    GrapheneError::new(
                        ErrorCode::NetworkStatusError,
                        ErrorKind::Network,
                        "failed to parse Modrinth project response",
                    )
                    .with_source(source)
                })?;

            normalize_project(dto)
        })
    }

    fn list_versions<'a>(
        &'a self,
        project_ref: &'a ContentProjectRef,
        filters: &'a ContentVersionFilter,
        operation: &'a OperationController,
    ) -> ContentProviderFuture<'a, Vec<ContentVersion>> {
        Box::pin(async move {
            let mut url = format!(
                "{}/project/{}/version?",
                self.config.base_url.trim_end_matches('/'),
                project_ref.project_id
            );

            if let Some(mv) = &filters.minecraft_version {
                let gv_json = serde_json::to_string(&vec![mv.clone()]).unwrap_or_default();
                url.push_str(&format!("game_versions={}&", percent_encode(&gv_json)));
            }
            if let Some(l) = &filters.loader {
                let l_json =
                    serde_json::to_string(&vec![l.to_string().to_lowercase()]).unwrap_or_default();
                url.push_str(&format!("loaders={}&", percent_encode(&l_json)));
            }

            let response = self
                .network
                .get_with_headers_bounded(
                    &url,
                    &self.headers(),
                    MAX_VERSIONS_RESPONSE_BYTES,
                    self.config.allow_http,
                    operation,
                )
                .await?;

            if response.status == 404 {
                return Err(GrapheneError::new(
                    ErrorCode::ContentProjectNotFound,
                    ErrorKind::Content,
                    format!("project {} not found on Modrinth", project_ref.project_id),
                ));
            }

            let dtos: Vec<ModrinthVersionDto> =
                serde_json::from_slice(&response.body).map_err(|source| {
                    GrapheneError::new(
                        ErrorCode::NetworkStatusError,
                        ErrorKind::Network,
                        "failed to parse Modrinth versions response",
                    )
                    .with_source(source)
                })?;

            dtos.into_iter().map(normalize_version).collect()
        })
    }

    fn get_version<'a>(
        &'a self,
        version_ref: &'a ContentVersionRef,
        operation: &'a OperationController,
    ) -> ContentProviderFuture<'a, ContentVersion> {
        Box::pin(async move {
            let url = format!(
                "{}/version/{}",
                self.config.base_url.trim_end_matches('/'),
                version_ref.version_id
            );

            let response = self
                .network
                .get_with_headers_bounded(
                    &url,
                    &self.headers(),
                    MAX_VERSION_RESPONSE_BYTES,
                    self.config.allow_http,
                    operation,
                )
                .await?;

            if response.status == 404 {
                return Err(GrapheneError::new(
                    ErrorCode::ContentVersionNotFound,
                    ErrorKind::Content,
                    format!("version {} not found on Modrinth", version_ref.version_id),
                ));
            }

            let dto: ModrinthVersionDto =
                serde_json::from_slice(&response.body).map_err(|source| {
                    GrapheneError::new(
                        ErrorCode::NetworkStatusError,
                        ErrorKind::Network,
                        "failed to parse Modrinth version response",
                    )
                    .with_source(source)
                })?;

            normalize_version(dto)
        })
    }

    fn match_files<'a>(
        &'a self,
        request: &'a FileMatchRequest,
        operation: &'a OperationController,
    ) -> ContentProviderFuture<'a, Vec<ContentFileMatch>> {
        Box::pin(async move {
            let mut results = Vec::with_capacity(request.items.len());
            let sha1_items: Vec<_> = request
                .items
                .iter()
                .filter_map(|it| it.sha1.as_ref().map(|s| (it, s.to_string())))
                .collect();

            if sha1_items.is_empty() {
                for it in &request.items {
                    results.push(ContentFileMatch {
                        item: it.clone(),
                        status: ExactMatchStatus::Unmatched,
                    });
                }
                return Ok(results);
            }

            let hashes: Vec<String> = sha1_items.iter().map(|(_, s)| s.clone()).collect();
            let body_json = serde_json::json!({
                "hashes": hashes,
                "algorithm": "sha1",
            });
            let body_bytes = serde_json::to_vec(&body_json).unwrap_or_default();

            let url = format!(
                "{}/version_files",
                self.config.base_url.trim_end_matches('/')
            );
            let response = self
                .network
                .post_json_with_headers_bounded(
                    &url,
                    &body_bytes,
                    &self.headers(),
                    MAX_MATCH_RESPONSE_BYTES,
                    self.config.allow_http,
                    operation,
                )
                .await?;

            let match_dto: ModrinthVersionFilesMatchDto = serde_json::from_slice(&response.body)
                .map_err(|source| {
                    GrapheneError::new(
                        ErrorCode::NetworkStatusError,
                        ErrorKind::Network,
                        "failed to parse Modrinth version files response",
                    )
                    .with_source(source)
                })?;

            for item in &request.items {
                if let Some(sha1_digest) = &item.sha1 {
                    let hex = sha1_digest.to_string();
                    if let Some(version_dto) = match_dto.matches.get(&hex) {
                        let normalized_ver = normalize_version(version_dto.clone())?;
                        let matched_file = normalized_ver
                            .files
                            .iter()
                            .find(|f| {
                                f.integrity.sha1().as_ref() == Some(sha1_digest)
                                    && f.size == item.size
                            })
                            .cloned();

                        if let Some(f) = matched_file {
                            results.push(ContentFileMatch {
                                item: item.clone(),
                                status: ExactMatchStatus::Matched {
                                    version: Box::new(normalized_ver),
                                    file: Box::new(f),
                                },
                            });
                            continue;
                        }
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
                    .then_with(|| b.version_number.cmp(&a.version_number))
            });

            Ok(candidates.into_iter().next())
        })
    }
}
