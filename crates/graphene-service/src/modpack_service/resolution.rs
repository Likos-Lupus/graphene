use crate::context::ServiceContext;
use graphene_content::{ContentFile, ContentProviderId, ContentVersion, ContentVersionRef};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, OperationController, Result};
use graphene_modpack::{FileSelection, PendingProviderFile, ProviderFileRef};
use graphene_providers::ContentProviderRegistry;
use std::sync::Arc;

/// A fully-resolved provider file with verified catalog metadata.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ResolvedPackFile {
    pending: PendingProviderFile,
    version: ContentVersion,
    file: ContentFile,
}

#[allow(dead_code)]
impl ResolvedPackFile {
    #[must_use]
    pub fn pending(&self) -> &PendingProviderFile {
        &self.pending
    }

    #[must_use]
    pub fn version(&self) -> &ContentVersion {
        &self.version
    }

    #[must_use]
    pub fn file(&self) -> &ContentFile {
        &self.file
    }

    #[must_use]
    pub fn selection(&self) -> &FileSelection {
        self.pending.selection()
    }

    #[must_use]
    pub fn provider_ref(&self) -> &ProviderFileRef {
        self.pending.provider_ref()
    }

    #[must_use]
    pub fn destination_filename(&self) -> &str {
        self.file.filename.as_str()
    }

    #[must_use]
    pub fn size(&self) -> u64 {
        self.file.size
    }
}

/// Resolves all pending provider files from a normalized modpack through the content provider
/// registry, returning fully-specified resolved files.
///
/// For CurseForge entries, this constructs an exact `ContentVersionRef` from the
/// `(project_id, file_id)` pair and queries the provider catalog. Each resolved file is
/// validated against the eight-point checklist from the roadmap:
///
/// 1. Returned version belongs to the requested project/file identity.
/// 2. Exactly the requested file is selected, never "latest".
/// 3. File is currently available.
/// 4. File has trustworthy integrity (SHA-1/SHA-256/SHA-512; never MD5/murmur2).
/// 5. (Deferred) Game-version/loader compatibility validated at plan composition.
/// 6. (Deferred) Provider provenance persisted into `LockedContentEntry` at commit time.
/// 7. (Deferred) Dependency information carried to plan composition, not solved here.
/// 8. Failure before staging if an exact referenced file cannot be obtained safely.
#[allow(dead_code)]
pub(crate) async fn resolve_pending_provider_files(
    context: &Arc<ServiceContext>,
    pending_files: &[PendingProviderFile],
    operation: &OperationController,
) -> Result<Vec<ResolvedPackFile>> {
    let mut resolved = Vec::with_capacity(pending_files.len());

    for pending in pending_files {
        let file = resolve_single_pending(&context.content_registry, pending, operation).await?;
        resolved.push(file);
    }

    Ok(resolved)
}

#[allow(dead_code)]
async fn resolve_single_pending(
    registry: &ContentProviderRegistry,
    pending: &PendingProviderFile,
    operation: &OperationController,
) -> Result<ResolvedPackFile> {
    match pending.provider_ref() {
        ProviderFileRef::CurseForge {
            project_id,
            file_id,
        } => {
            let provider_id = ContentProviderId::new(ContentProviderId::CURSEFORGE)?;
            let provider = registry.get(&provider_id)?;

            let version_ref =
                ContentVersionRef::new(provider_id, project_id.as_str(), file_id.as_str())?;

            let version = provider.get_version(&version_ref, operation).await?;

            // Checklist 1: verify version still belongs to the requested project/file identity.
            if version.version_ref.project_id != *project_id {
                return Err(GrapheneError::new(
                    ErrorCode::PackProviderResolutionFailed,
                    ErrorKind::Modpack,
                    format!(
                        "resolved version project {}/{} does not match requested project {project_id}",
                        version.version_ref.provider, version.version_ref.project_id,
                    ),
                ));
            }
            if version.version_ref.version_id != *file_id {
                return Err(GrapheneError::new(
                    ErrorCode::PackProviderResolutionFailed,
                    ErrorKind::Modpack,
                    format!(
                        "resolved version id {} does not match requested file {file_id}",
                        version.version_ref.version_id,
                    ),
                ));
            }

            // Checklist 3: file must be currently available.
            if !version.available {
                return Err(GrapheneError::new(
                    ErrorCode::ContentFileUnavailable,
                    ErrorKind::Modpack,
                    format!("curseforge file {project_id}/{file_id} is not currently available",),
                ));
            }

            // Checklist 2+4: select exactly the requested file, require trustworthy integrity.
            let file = find_exact_file(&version, file_id)?;

            // Checklist 4: require trustworthy integrity (SHA-1/SHA-256/SHA-512).
            if !file.is_verifiable() {
                return Err(GrapheneError::new(
                    ErrorCode::PackArtifactUnverifiable,
                    ErrorKind::Modpack,
                    format!(
                        "curseforge file {project_id}/{file_id} has no trustworthy integrity \
                         declaration (MD5 and Murmur2 never satisfy integrity)",
                    ),
                ));
            }

            Ok(ResolvedPackFile {
                pending: pending.clone(),
                version,
                file,
            })
        }
        other => Err(GrapheneError::new(
            ErrorCode::PackProviderResolutionFailed,
            ErrorKind::Modpack,
            format!(
                "unsupported provider file reference kind: {:?}",
                std::mem::discriminant(other),
            ),
        )),
    }
}

/// Selects exactly the requested file from the version's file list by matching on
/// the file_ref's version_id (which corresponds to the CurseForge file ID).
///
/// CurseForge maps `(project_id, file_id)` to a single `ContentVersion` containing
/// exactly one file, but we verify explicitly rather than relying on that invariant.
#[allow(dead_code)]
fn find_exact_file(version: &ContentVersion, requested_file_id: &str) -> Result<ContentFile> {
    let candidates: Vec<&ContentFile> = version
        .files
        .iter()
        .filter(|f| f.file_ref.version_id == requested_file_id)
        .collect();

    match candidates.len() {
        0 => Err(GrapheneError::new(
            ErrorCode::ContentVersionNotFound,
            ErrorKind::Modpack,
            format!(
                "resolved version {} contains no file with id {requested_file_id}",
                version.version_ref,
            ),
        )),
        1 => Ok(candidates.into_iter().next().unwrap().clone()),
        _ => Err(GrapheneError::new(
            ErrorCode::ContentAmbiguousMatch,
            ErrorKind::Modpack,
            format!(
                "resolved version {} contains {} files with id {requested_file_id}, expected exactly 1",
                version.version_ref,
                candidates.len(),
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphene_content::{
        ContentFileRef, ContentProviderCapabilities, ContentProviderFuture, ContentSearchQuery,
        ContentVersionFilter, FileMatchRequest, InstanceContentContext, ReleaseChannelPolicy,
        model::{release::ReleaseChannel, version::EnvironmentSupport},
        provider::match_request::ContentFileMatch,
    };
    use graphene_core::{ArtifactIntegrity, ArtifactSource};
    use graphene_modpack::{FileSelection, ProviderFileRef};
    use graphene_providers::ContentProviderRegistry;
    use std::collections::BTreeMap;

    fn provider_id() -> ContentProviderId {
        ContentProviderId::new(ContentProviderId::CURSEFORGE).unwrap()
    }

    fn make_version_ref(project_id: &str, file_id: &str) -> ContentVersionRef {
        ContentVersionRef::new(provider_id(), project_id, file_id).unwrap()
    }

    fn make_file_ref(project_id: &str, file_id: &str, filename: &str) -> ContentFileRef {
        ContentFileRef::new(provider_id(), project_id, file_id, filename).unwrap()
    }

    fn make_content_file(
        project_id: &str,
        file_id: &str,
        filename: &str,
        size: u64,
        sha1: [u8; 20],
    ) -> ContentFile {
        ContentFile {
            file_ref: make_file_ref(project_id, file_id, filename),
            filename: filename.to_string(),
            role: graphene_content::FileRole::Primary,
            size,
            integrity: ArtifactIntegrity::none()
                .with_sha1(graphene_core::Sha1Digest::from_bytes(sha1)),
            sources: vec![ArtifactSource::new(format!(
                "https://edge.forgecdn.net/files/{}/{}/{}",
                file_id.get(..4).unwrap_or(file_id),
                file_id.get(4..).unwrap_or("0"),
                filename,
            ))],
            available: true,
            murmur2_fingerprint: None,
            md5: None,
        }
    }

    fn make_content_version(
        project_id: &str,
        file_id: &str,
        filename: &str,
        size: u64,
        sha1: [u8; 20],
    ) -> ContentVersion {
        let file = make_content_file(project_id, file_id, filename, size, sha1);
        let version_ref = make_version_ref(project_id, file_id);

        ContentVersion {
            version_ref,
            version_number: format!("v{file_id}"),
            display_name: format!("{filename} v{file_id}"),
            release_channel: ReleaseChannel::Release,
            game_versions: vec!["1.20.1".to_string()],
            loaders: Vec::new(),
            environment: EnvironmentSupport::Both,
            dependencies: Vec::new(),
            files: vec![file],
            date_published: Some("2024-01-01T00:00:00Z".to_string()),
            downloads: Some(1000),
            available: true,
        }
    }

    fn make_pending_curseforge(project_id: &str, file_id: &str) -> PendingProviderFile {
        PendingProviderFile::new(
            ProviderFileRef::CurseForge {
                project_id: project_id.to_string(),
                file_id: file_id.to_string(),
            },
            FileSelection::Required,
        )
        .unwrap()
    }

    #[derive(Debug)]
    struct FakeProvider {
        id: ContentProviderId,
        versions: BTreeMap<String, ContentVersion>,
        mismatch_version: Option<ContentVersion>,
    }

    impl FakeProvider {
        fn new() -> Self {
            Self {
                id: provider_id(),
                versions: BTreeMap::new(),
                mismatch_version: None,
            }
        }

        fn with_version(mut self, version: ContentVersion) -> Self {
            let key = format!(
                "{}:{}",
                version.version_ref.project_id, version.version_ref.version_id
            );
            self.versions.insert(key, version);
            self
        }

        fn with_mismatch_version(mut self, version: ContentVersion) -> Self {
            self.mismatch_version = Some(version);
            self
        }
    }

    impl graphene_content::ContentProvider for FakeProvider {
        fn id(&self) -> &ContentProviderId {
            &self.id
        }

        fn capabilities(&self) -> ContentProviderCapabilities {
            ContentProviderCapabilities::curseforge()
        }

        fn search<'a>(
            &'a self,
            _query: &'a ContentSearchQuery,
            _operation: &'a OperationController,
        ) -> ContentProviderFuture<'a, graphene_content::ContentSearchPage> {
            Box::pin(async {
                Ok(graphene_content::ContentSearchPage {
                    hits: Vec::new(),
                    offset: 0,
                    limit: 0,
                    total_hits: Some(0),
                })
            })
        }

        fn project<'a>(
            &'a self,
            _project_ref: &'a graphene_content::ContentProjectRef,
            _operation: &'a OperationController,
        ) -> ContentProviderFuture<'a, graphene_content::ContentProject> {
            unimplemented!("not needed for resolution tests")
        }

        fn list_versions<'a>(
            &'a self,
            _project_ref: &'a graphene_content::ContentProjectRef,
            _filters: &'a ContentVersionFilter,
            _operation: &'a OperationController,
        ) -> ContentProviderFuture<'a, Vec<ContentVersion>> {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn get_version<'a>(
            &'a self,
            version_ref: &'a ContentVersionRef,
            _operation: &'a OperationController,
        ) -> ContentProviderFuture<'a, ContentVersion> {
            let key = format!("{}:{}", version_ref.project_id, version_ref.version_id);
            let result = self
                .versions
                .get(&key)
                .cloned()
                .or_else(|| self.mismatch_version.clone())
                .ok_or_else(|| {
                    GrapheneError::new(
                        ErrorCode::ContentVersionNotFound,
                        ErrorKind::Content,
                        format!("version {key} not found in fake provider"),
                    )
                });
            Box::pin(async { result })
        }

        fn match_files<'a>(
            &'a self,
            _request: &'a FileMatchRequest,
            _operation: &'a OperationController,
        ) -> ContentProviderFuture<'a, Vec<ContentFileMatch>> {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn find_update<'a>(
            &'a self,
            _current_file: &'a graphene_content::ContentFileRef,
            _context: &'a InstanceContentContext,
            _policy: ReleaseChannelPolicy,
            _operation: &'a OperationController,
        ) -> ContentProviderFuture<'a, Option<ContentVersion>> {
            Box::pin(async { Ok(None) })
        }
    }

    fn make_registry(provider: FakeProvider) -> ContentProviderRegistry {
        let mut registry = ContentProviderRegistry::new();
        registry
            .register(Arc::new(provider) as Arc<dyn graphene_content::ContentProvider>)
            .unwrap();
        registry
    }

    fn null_operation() -> OperationController {
        graphene_core::OperationRegistry::new(64)
            .unwrap()
            .create("test-op")
    }

    // ---- Tests using resolve_single_pending directly ----

    #[tokio::test]
    async fn resolves_single_curseforge_file_successfully() {
        let sha1: [u8; 20] = [0xAA; 20];
        let version = make_content_version("42", "999", "sodium-0.5.1.jar", 1024, sha1);
        let registry = make_registry(FakeProvider::new().with_version(version));
        let pending = make_pending_curseforge("42", "999");
        let op = null_operation();

        let result = resolve_single_pending(&registry, &pending, &op)
            .await
            .unwrap();

        assert_eq!(result.file().filename, "sodium-0.5.1.jar");
        assert_eq!(result.file().size, 1024);
        assert!(result.file().is_verifiable());
        assert_eq!(result.version().version_ref.project_id, "42");
        assert_eq!(result.version().version_ref.version_id, "999");
    }

    #[tokio::test]
    async fn resolves_multiple_files_independently() {
        let sha1_a: [u8; 20] = [0xBB; 20];
        let sha1_b: [u8; 20] = [0xCC; 20];
        let v_a = make_content_version("1", "100", "mod-a.jar", 500, sha1_a);
        let v_b = make_content_version("2", "200", "mod-b.jar", 750, sha1_b);
        let registry = make_registry(FakeProvider::new().with_version(v_a).with_version(v_b));

        let pending_a = make_pending_curseforge("1", "100");
        let pending_b = make_pending_curseforge("2", "200");
        let op = null_operation();

        let result_a = resolve_single_pending(&registry, &pending_a, &op)
            .await
            .unwrap();
        let result_b = resolve_single_pending(&registry, &pending_b, &op)
            .await
            .unwrap();

        assert_eq!(result_a.file().filename, "mod-a.jar");
        assert_eq!(result_a.size(), 500);
        assert_eq!(result_b.file().filename, "mod-b.jar");
        assert_eq!(result_b.size(), 750);
    }

    #[tokio::test]
    async fn fails_when_provider_version_not_found() {
        let registry = make_registry(FakeProvider::new());
        let pending = make_pending_curseforge("42", "999");
        let op = null_operation();

        let err = resolve_single_pending(&registry, &pending, &op)
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::ContentVersionNotFound);
    }

    #[tokio::test]
    async fn fails_when_version_not_available() {
        let sha1: [u8; 20] = [0xDD; 20];
        let mut version = make_content_version("42", "999", "mod.jar", 1024, sha1);
        version.available = false;
        let registry = make_registry(FakeProvider::new().with_version(version));
        let pending = make_pending_curseforge("42", "999");
        let op = null_operation();

        let err = resolve_single_pending(&registry, &pending, &op)
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::ContentFileUnavailable);
    }

    #[tokio::test]
    async fn fails_when_file_not_verifiable() {
        let file_ref = make_file_ref("42", "999", "mod.jar");
        let version_ref = make_version_ref("42", "999");

        let file = ContentFile {
            file_ref,
            filename: "mod.jar".to_string(),
            role: graphene_content::FileRole::Primary,
            size: 1024,
            integrity: ArtifactIntegrity::none(),
            sources: vec![ArtifactSource::new(
                "https://example.com/mod.jar".to_string(),
            )],
            available: true,
            murmur2_fingerprint: Some(12345),
            md5: Some("abc123".to_string()),
        };

        let version = ContentVersion {
            version_ref,
            version_number: "v999".to_string(),
            display_name: "mod.jar v999".to_string(),
            release_channel: ReleaseChannel::Release,
            game_versions: vec!["1.20.1".to_string()],
            loaders: Vec::new(),
            environment: EnvironmentSupport::Both,
            dependencies: Vec::new(),
            files: vec![file],
            date_published: Some("2024-01-01T00:00:00Z".to_string()),
            downloads: Some(1000),
            available: true,
        };

        let registry = make_registry(FakeProvider::new().with_version(version));
        let pending = make_pending_curseforge("42", "999");
        let op = null_operation();

        let err = resolve_single_pending(&registry, &pending, &op)
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::PackArtifactUnverifiable);
    }

    #[tokio::test]
    async fn fails_when_project_id_mismatch() {
        let sha1: [u8; 20] = [0xEE; 20];
        let mismatch_version = make_content_version("WRONG_PROJECT", "999", "mod.jar", 1024, sha1);
        let registry = make_registry(FakeProvider::new().with_mismatch_version(mismatch_version));
        let pending = make_pending_curseforge("42", "999");
        let op = null_operation();

        let err = resolve_single_pending(&registry, &pending, &op)
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::PackProviderResolutionFailed);
        assert!(err.message().contains("project"));
    }

    #[tokio::test]
    async fn fails_when_file_id_mismatch() {
        let sha1: [u8; 20] = [0xFF; 20];
        let mismatch_version = make_content_version("42", "WRONG_FILE", "mod.jar", 1024, sha1);
        let registry = make_registry(FakeProvider::new().with_mismatch_version(mismatch_version));
        let pending = make_pending_curseforge("42", "999");
        let op = null_operation();

        let err = resolve_single_pending(&registry, &pending, &op)
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::PackProviderResolutionFailed);
        assert!(err.message().contains("file"));
    }

    #[tokio::test]
    async fn resolved_file_preserves_selection() {
        let sha1: [u8; 20] = [0x11; 20];
        let version = make_content_version("42", "999", "mod.jar", 1024, sha1);
        let registry = make_registry(FakeProvider::new().with_version(version));

        let pending = PendingProviderFile::new(
            ProviderFileRef::CurseForge {
                project_id: "42".to_string(),
                file_id: "999".to_string(),
            },
            FileSelection::Optional {
                choice_id: "opt-a".to_string(),
            },
        )
        .unwrap();
        let op = null_operation();

        let result = resolve_single_pending(&registry, &pending, &op)
            .await
            .unwrap();

        match result.selection() {
            FileSelection::Optional { choice_id } => assert_eq!(choice_id, "opt-a"),
            other => panic!("expected Optional selection, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn resolved_file_has_verifiable_sha1() {
        let sha1: [u8; 20] = [0xAA; 20];
        let version = make_content_version("42", "999", "mod.jar", 1024, sha1);
        let registry = make_registry(FakeProvider::new().with_version(version));
        let pending = make_pending_curseforge("42", "999");
        let op = null_operation();

        let result = resolve_single_pending(&registry, &pending, &op)
            .await
            .unwrap();

        assert_eq!(
            result.file().integrity.sha1(),
            Some(graphene_core::Sha1Digest::from_bytes(sha1))
        );
    }

    #[test]
    fn find_exact_file_returns_matching_file() {
        let sha1: [u8; 20] = [0xAA; 20];
        let version = make_content_version("42", "999", "mod.jar", 1024, sha1);
        let file = find_exact_file(&version, "999").unwrap();
        assert_eq!(file.filename, "mod.jar");
        assert_eq!(file.size, 1024);
    }

    #[test]
    fn find_exact_file_returns_error_for_missing_id() {
        let sha1: [u8; 20] = [0xAA; 20];
        let version = make_content_version("42", "999", "mod.jar", 1024, sha1);
        let err = find_exact_file(&version, "WRONG").unwrap_err();
        assert_eq!(err.code, ErrorCode::ContentVersionNotFound);
    }

    #[test]
    fn find_exact_file_returns_error_for_duplicate_ids() {
        let sha1: [u8; 20] = [0xAA; 20];
        let file_a = make_content_file("42", "999", "mod-a.jar", 500, sha1);
        let file_b = make_content_file("42", "999", "mod-b.jar", 500, sha1);

        let version = ContentVersion {
            version_ref: make_version_ref("42", "999"),
            version_number: "v999".to_string(),
            display_name: "v999".to_string(),
            release_channel: ReleaseChannel::Release,
            game_versions: Vec::new(),
            loaders: Vec::new(),
            environment: EnvironmentSupport::Both,
            dependencies: Vec::new(),
            files: vec![file_a, file_b],
            date_published: None,
            downloads: None,
            available: true,
        };

        let err = find_exact_file(&version, "999").unwrap_err();
        assert_eq!(err.code, ErrorCode::ContentAmbiguousMatch);
    }
}
