use crate::content::curseforge::dto::{
    CurseForgeFileDependencyDto, CurseForgeFileDto, CurseForgeFilesResponseDto, CurseForgeModDto,
    CurseForgeSearchResponseDto,
};
use graphene_content::{
    ContentDependency, ContentFile, ContentFileRef, ContentKind, ContentProject, ContentProjectRef,
    ContentProviderId, ContentSearchHit, ContentSearchPage, ContentVersion, ContentVersionRef,
    DependencyRelation, DependencyTarget, EnvironmentSupport, FileRole, ReleaseChannel,
};
use graphene_core::{ArtifactIntegrity, ArtifactSource, Result, Sha1Digest};
use graphene_minecraft::LoaderKind;

pub(super) fn provider_id() -> ContentProviderId {
    ContentProviderId::new(ContentProviderId::CURSEFORGE).expect("static curseforge id")
}

pub(super) fn normalize_release_channel(release_type: u32) -> ReleaseChannel {
    match release_type {
        1 => ReleaseChannel::Release,
        2 => ReleaseChannel::Beta,
        3 => ReleaseChannel::Alpha,
        _ => ReleaseChannel::Unknown,
    }
}

pub(super) fn parse_loaders_and_mc_versions(
    game_versions: &[String],
) -> (Vec<LoaderKind>, Vec<String>) {
    let mut loaders = Vec::new();
    let mut mc_versions = Vec::new();

    for gv in game_versions {
        let lower = gv.to_ascii_lowercase();
        match lower.as_str() {
            "fabric" => {
                if !loaders.contains(&LoaderKind::Fabric) {
                    loaders.push(LoaderKind::Fabric);
                }
            }
            "forge" => {
                if !loaders.contains(&LoaderKind::Forge) {
                    loaders.push(LoaderKind::Forge);
                }
            }
            "neoforge" => {
                if !loaders.contains(&LoaderKind::NeoForge) {
                    loaders.push(LoaderKind::NeoForge);
                }
            }
            _ => {
                if !mc_versions.contains(gv) {
                    mc_versions.push(gv.clone());
                }
            }
        }
    }

    (loaders, mc_versions)
}

pub(super) fn normalize_search_page(dto: CurseForgeSearchResponseDto) -> Result<ContentSearchPage> {
    let pid = provider_id();
    let hits = dto
        .data
        .into_iter()
        .map(|mod_dto| normalize_search_hit(mod_dto, &pid))
        .collect::<Result<Vec<_>>>()?;

    let (offset, limit, total_hits) = if let Some(p) = dto.pagination {
        (
            p.index.unwrap_or(0),
            p.page_size.unwrap_or(10),
            p.total_count,
        )
    } else {
        (0, 10, None)
    };

    Ok(ContentSearchPage {
        hits,
        offset,
        limit,
        total_hits,
    })
}

fn normalize_search_hit(
    dto: CurseForgeModDto,
    pid: &ContentProviderId,
) -> Result<ContentSearchHit> {
    let project_ref = ContentProjectRef::new(pid.clone(), dto.id.to_string())?;
    let authors = dto
        .authors
        .map(|list| list.into_iter().map(|a| a.name).collect())
        .unwrap_or_default();
    let categories = dto
        .categories
        .map(|list| list.into_iter().map(|c| c.name).collect())
        .unwrap_or_default();

    Ok(ContentSearchHit {
        project_ref,
        kind: ContentKind::Mod,
        slug: dto.slug,
        title: dto.name,
        summary: dto.summary.unwrap_or_default(),
        icon_url: dto.logo.and_then(|l| l.thumbnail_url.or(l.url)),
        authors,
        categories,
        downloads: dto.download_count,
        follows: dto.thumbs_up_count,
        game_versions: Vec::new(),
        loaders: Vec::new(),
    })
}

pub(super) fn normalize_project(dto: CurseForgeModDto) -> Result<ContentProject> {
    let pid = provider_id();
    let project_ref = ContentProjectRef::new(pid, dto.id.to_string())?;

    let authors = dto
        .authors
        .map(|list| list.into_iter().map(|a| a.name).collect())
        .unwrap_or_default();
    let categories = dto
        .categories
        .map(|list| list.into_iter().map(|c| c.name).collect())
        .unwrap_or_default();

    let (website_url, wiki_url, issues_url, source_url) = if let Some(links) = dto.links {
        (
            links.website_url,
            links.wiki_url,
            links.issues_url,
            links.source_url,
        )
    } else {
        (None, None, None, None)
    };

    Ok(ContentProject {
        project_ref,
        kind: ContentKind::Mod,
        slug: dto.slug,
        title: dto.name,
        summary: dto.summary.unwrap_or_default(),
        icon_url: dto.logo.and_then(|l| l.thumbnail_url.or(l.url)),
        website_url,
        source_url,
        issues_url,
        wiki_url,
        authors,
        categories,
        downloads: dto.download_count,
        follows: dto.thumbs_up_count,
        client_side: EnvironmentSupport::Both,
        server_side: EnvironmentSupport::Both,
        game_versions: Vec::new(),
        loaders: Vec::new(),
    })
}

pub(super) fn normalize_file(
    dto: CurseForgeFileDto,
    pid: &ContentProviderId,
) -> Result<(ContentVersion, ContentFile)> {
    let project_id_str = dto.mod_id.to_string();
    let file_id_str = dto.id.to_string();

    let version_ref = ContentVersionRef::new(pid.clone(), &project_id_str, &file_id_str)?;
    let file_ref = ContentFileRef::new(pid.clone(), &project_id_str, &file_id_str, &dto.file_name)?;

    let gv_list = dto.game_versions.unwrap_or_default();
    let (loaders, game_versions) = parse_loaders_and_mc_versions(&gv_list);

    let mut dependencies = Vec::new();
    if let Some(deps_dto) = dto.dependencies {
        for dep in deps_dto {
            dependencies.push(normalize_dependency(dep, pid)?);
        }
    }

    let mut sha1_digest = None;
    let mut md5_str = None;

    if let Some(hashes) = dto.hashes {
        for h in hashes {
            if h.algo == 1 {
                if let Ok(d) = h.value.parse::<Sha1Digest>() {
                    sha1_digest = Some(d);
                }
            } else if h.algo == 2 {
                md5_str = Some(h.value);
            }
        }
    }

    let mut integrity = ArtifactIntegrity::none();
    if let Some(sha1) = sha1_digest {
        integrity = integrity.with_sha1(sha1);
    }

    let sources = if let Some(url) = dto.download_url {
        vec![ArtifactSource::new(url)]
    } else {
        Vec::new()
    };

    let available = dto.is_available.unwrap_or(true) && dto.file_status.unwrap_or(4) == 4;

    let content_file = ContentFile {
        file_ref,
        filename: dto.file_name,
        role: FileRole::Primary,
        size: dto.file_length,
        integrity,
        sources,
        available,
        murmur2_fingerprint: dto.package_fingerprint,
        md5: md5_str,
    };

    let content_version = ContentVersion {
        version_ref,
        version_number: dto.display_name.clone(),
        display_name: dto.display_name,
        release_channel: normalize_release_channel(dto.release_type),
        game_versions,
        loaders,
        environment: EnvironmentSupport::Both,
        dependencies,
        files: vec![content_file.clone()],
        date_published: dto.file_date,
        downloads: dto.download_count,
        available,
    };

    Ok((content_version, content_file))
}

fn normalize_dependency(
    dto: CurseForgeFileDependencyDto,
    pid: &ContentProviderId,
) -> Result<ContentDependency> {
    let relation = match dto.relation_type {
        1 | 6 => DependencyRelation::Embedded,
        2 | 4 => DependencyRelation::Optional,
        3 => DependencyRelation::Required,
        5 => DependencyRelation::Incompatible,
        _ => DependencyRelation::Unknown,
    };

    let target =
        DependencyTarget::Project(ContentProjectRef::new(pid.clone(), dto.mod_id.to_string())?);

    Ok(ContentDependency::new(target, relation))
}

pub(super) fn normalize_files_response(
    dto: CurseForgeFilesResponseDto,
) -> Result<Vec<ContentVersion>> {
    let pid = provider_id();
    let mut versions = Vec::new();
    for file_dto in dto.data {
        let (v, _) = normalize_file(file_dto, &pid)?;
        versions.push(v);
    }
    Ok(versions)
}
