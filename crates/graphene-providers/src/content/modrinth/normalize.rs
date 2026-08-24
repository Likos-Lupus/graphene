use crate::content::modrinth::dto::{
    ModrinthDependencyDto, ModrinthProjectDto, ModrinthSearchHitDto, ModrinthSearchResponseDto,
    ModrinthVersionDto, ModrinthVersionFileDto,
};
use graphene_content::{
    ContentDependency, ContentFile, ContentFileRef, ContentKind, ContentProject, ContentProjectRef,
    ContentProviderId, ContentSearchHit, ContentSearchPage, ContentVersion, ContentVersionRef,
    DependencyRelation, DependencyTarget, EnvironmentSupport, FileRole, ReleaseChannel,
};
use graphene_core::{ArtifactIntegrity, ArtifactSource, Result, Sha1Digest};
use graphene_minecraft::LoaderKind;

pub(super) fn provider_id() -> ContentProviderId {
    ContentProviderId::new(ContentProviderId::MODRINTH).expect("static modrinth id")
}

pub(super) fn normalize_loader(s: &str) -> Option<LoaderKind> {
    match s.to_ascii_lowercase().as_str() {
        "fabric" => Some(LoaderKind::Fabric),
        "forge" => Some(LoaderKind::Forge),
        "neoforge" => Some(LoaderKind::NeoForge),
        _ => None,
    }
}

pub(super) fn normalize_release_channel(s: Option<&str>) -> ReleaseChannel {
    match s.map(str::to_ascii_lowercase).as_deref() {
        Some("release") => ReleaseChannel::Release,
        Some("beta") => ReleaseChannel::Beta,
        Some("alpha") => ReleaseChannel::Alpha,
        _ => ReleaseChannel::Unknown,
    }
}

pub(super) fn normalize_environment(s: Option<&str>) -> EnvironmentSupport {
    match s.map(str::to_ascii_lowercase).as_deref() {
        Some("required") | Some("optional") => EnvironmentSupport::Both,
        Some("unsupported") => EnvironmentSupport::ServerOnly,
        _ => EnvironmentSupport::Unknown,
    }
}

pub(super) fn normalize_search_page(dto: ModrinthSearchResponseDto) -> Result<ContentSearchPage> {
    let pid = provider_id();
    let hits = dto
        .hits
        .into_iter()
        .map(|hit| normalize_search_hit(hit, &pid))
        .collect::<Result<Vec<_>>>()?;

    Ok(ContentSearchPage {
        hits,
        offset: dto.offset.unwrap_or(0),
        limit: dto.limit.unwrap_or(10),
        total_hits: dto.total_hits,
    })
}

fn normalize_search_hit(
    dto: ModrinthSearchHitDto,
    pid: &ContentProviderId,
) -> Result<ContentSearchHit> {
    let project_ref = ContentProjectRef::new(pid.clone(), dto.project_id)?;
    let mut loaders = Vec::new();
    if let Some(cats) = &dto.categories {
        for cat in cats {
            if let Some(l) = normalize_loader(cat)
                && !loaders.contains(&l)
            {
                loaders.push(l);
            }
        }
    }

    Ok(ContentSearchHit {
        project_ref,
        kind: ContentKind::Mod,
        slug: dto.slug,
        title: dto.title,
        summary: dto.description.unwrap_or_default(),
        icon_url: dto.icon_url,
        authors: dto.author.into_iter().collect(),
        categories: dto.categories.unwrap_or_default(),
        downloads: dto.downloads,
        follows: dto.follows,
        game_versions: dto.versions.unwrap_or_default(),
        loaders,
    })
}

pub(super) fn normalize_project(dto: ModrinthProjectDto) -> Result<ContentProject> {
    let pid = provider_id();
    let project_ref = ContentProjectRef::new(pid, dto.id)?;

    let mut loaders = Vec::new();
    if let Some(loaders_dto) = dto.loaders {
        for l_str in loaders_dto {
            if let Some(l) = normalize_loader(&l_str)
                && !loaders.contains(&l)
            {
                loaders.push(l);
            }
        }
    }

    let client_side = normalize_environment(dto.client_side.as_deref());
    let server_side = normalize_environment(dto.server_side.as_deref());

    Ok(ContentProject {
        project_ref,
        kind: ContentKind::Mod,
        slug: dto.slug,
        title: dto.title,
        summary: dto.description.unwrap_or_default(),
        icon_url: dto.icon_url,
        website_url: dto.discord_url,
        source_url: dto.source_url,
        issues_url: dto.issues_url,
        wiki_url: dto.wiki_url,
        authors: Vec::new(),
        categories: dto.categories.unwrap_or_default(),
        downloads: dto.downloads,
        follows: dto.followers,
        client_side,
        server_side,
        game_versions: dto.game_versions.unwrap_or_default(),
        loaders,
    })
}

pub(super) fn normalize_version(dto: ModrinthVersionDto) -> Result<ContentVersion> {
    let pid = provider_id();
    let version_ref = ContentVersionRef::new(pid.clone(), &dto.project_id, &dto.id)?;

    let mut loaders = Vec::new();
    for l_str in &dto.loaders {
        if let Some(l) = normalize_loader(l_str)
            && !loaders.contains(&l)
        {
            loaders.push(l);
        }
    }

    let mut dependencies = Vec::new();
    if let Some(deps_dto) = dto.dependencies {
        for dep in deps_dto {
            dependencies.push(normalize_dependency(dep, &pid)?);
        }
    }

    let mut files = Vec::new();
    for file_dto in dto.files {
        files.push(normalize_file(file_dto, &version_ref)?);
    }

    let available = dto.status.as_deref() != Some("deleted")
        && dto.status.as_deref() != Some("archived")
        && dto.status.as_deref() != Some("rejected");

    Ok(ContentVersion {
        version_ref,
        version_number: dto.version_number,
        display_name: dto.name.unwrap_or_else(|| dto.id.clone()),
        release_channel: normalize_release_channel(dto.version_type.as_deref()),
        game_versions: dto.game_versions,
        loaders,
        environment: EnvironmentSupport::Both,
        dependencies,
        files,
        date_published: dto.date_published,
        downloads: dto.downloads,
        available,
    })
}

fn normalize_dependency(
    dto: ModrinthDependencyDto,
    pid: &ContentProviderId,
) -> Result<ContentDependency> {
    let relation = match dto.dependency_type.to_ascii_lowercase().as_str() {
        "required" => DependencyRelation::Required,
        "optional" => DependencyRelation::Optional,
        "incompatible" => DependencyRelation::Incompatible,
        "embedded" => DependencyRelation::Embedded,
        _ => DependencyRelation::Unknown,
    };

    let target = if let (Some(pid_str), Some(vid_str)) = (&dto.project_id, &dto.version_id) {
        DependencyTarget::Version(ContentVersionRef::new(pid.clone(), pid_str, vid_str)?)
    } else if let Some(pid_str) = &dto.project_id {
        DependencyTarget::Project(ContentProjectRef::new(pid.clone(), pid_str)?)
    } else if let Some(hint) = dto.file_name {
        DependencyTarget::FilenameHint(hint)
    } else {
        DependencyTarget::FilenameHint("unspecified".to_string())
    };

    Ok(ContentDependency::new(target, relation))
}

fn normalize_file(dto: ModrinthVersionFileDto, vref: &ContentVersionRef) -> Result<ContentFile> {
    let file_ref = ContentFileRef::new(
        vref.provider.clone(),
        &vref.project_id,
        &vref.version_id,
        &dto.filename,
    )?;

    let mut integrity = ArtifactIntegrity::none();
    if let Ok(digest) = dto
        .hashes
        .sha1
        .as_deref()
        .unwrap_or_default()
        .parse::<Sha1Digest>()
    {
        integrity = integrity.with_sha1(digest);
    }

    let role = if dto.primary.unwrap_or(false) {
        FileRole::Primary
    } else {
        FileRole::Alternative
    };

    let sources = vec![ArtifactSource::new(&dto.url)];

    Ok(ContentFile {
        file_ref,
        filename: dto.filename,
        role,
        size: dto.size,
        integrity,
        sources,
        available: true,
        murmur2_fingerprint: None,
        md5: None,
    })
}
