use crate::content::modrinth::dto::{
    ModrinthProjectDto, ModrinthSearchResponseDto, ModrinthVersionDto,
};
use crate::content::modrinth::normalize::{
    normalize_project, normalize_search_page, normalize_version,
};
use graphene_content::{
    DependencyRelation, DependencyTarget, EnvironmentSupport, FileRole, ReleaseChannel,
};
use graphene_core::ErrorCode;
use graphene_minecraft::LoaderKind;

fn search_fixture() -> serde_json::Value {
    serde_json::json!({
        "hits": [{
            "project_id": "AABBCCDD",
            "slug": "sodium",
            "title": "Sodium",
            "description": "Modern rendering engine",
            "icon_url": "https://cdn.modrinth.com/sodium.png",
            "author": "jellysquid3",
            "categories": ["fabric", "optimization"],
            "downloads": 4_000_000u64,
            "follows": 12_000u64,
            "versions": ["1.21.1", "1.21"]
        }],
        "offset": 0,
        "limit": 10,
        "total_hits": 1
    })
}

#[test]
fn search_page_normalizes_hits_and_pagination() {
    let dto: ModrinthSearchResponseDto =
        serde_json::from_value(search_fixture()).expect("fixture dto");
    let page = normalize_search_page(dto).expect("normalized page");

    assert_eq!(page.hits.len(), 1);
    assert_eq!(page.hits[0].slug, "sodium");
    assert_eq!(page.hits[0].project_ref.project_id, "AABBCCDD");
    assert_eq!(page.hits[0].project_ref.provider.as_str(), "modrinth");
    assert_eq!(page.offset, 0);
    assert_eq!(page.limit, 10);
    assert_eq!(page.total_hits, Some(1));
    assert!(page.hits[0].loaders.contains(&LoaderKind::Fabric));
}

#[test]
fn version_normalizes_files_dependencies_and_channel() {
    let dto: ModrinthVersionDto = serde_json::from_value(serde_json::json!({
        "id": "VERSION1",
        "project_id": "AABBCCDD",
        "name": "Sodium 0.5.8",
        "version_number": "0.5.8",
        "version_type": "release",
        "game_versions": ["1.21.1"],
        "loaders": ["fabric"],
        "dependencies": [
            {"project_id": "DEADBEEF", "version_id": null, "dependency_type": "required"},
            {"project_id": "CAFEBABE", "version_id": "OPTVRSN", "dependency_type": "optional"},
            {"project_id": "BADCODE", "version_id": null, "dependency_type": "incompatible"},
            {"project_id": "EMBEDDED", "version_id": null, "dependency_type": "embedded"}
        ],
        "files": [{
            "hashes": {"sha1": "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12"},
            "url": "https://cdn.modrinth.com/sodium.jar",
            "filename": "sodium-0.5.8.jar",
            "primary": true,
            "size": 1024
        }],
        "date_published": "2024-06-01T00:00:00Z",
        "downloads": 100,
        "status": "listed"
    }))
    .expect("fixture dto");

    let version = normalize_version(dto).expect("normalized version");
    assert_eq!(version.version_ref.version_id, "VERSION1");
    assert_eq!(version.release_channel, ReleaseChannel::Release);
    assert_eq!(version.loaders, vec![LoaderKind::Fabric]);
    assert_eq!(version.game_versions, vec!["1.21.1"]);
    assert_eq!(version.dependencies.len(), 4);
    assert_eq!(
        version.dependencies[0].relation,
        DependencyRelation::Required
    );
    assert!(matches!(
        &version.dependencies[0].target,
        DependencyTarget::Project(p) if p.project_id == "DEADBEEF"
    ));
    assert_eq!(
        version.dependencies[1].relation,
        DependencyRelation::Optional
    );
    assert_eq!(
        version.dependencies[2].relation,
        DependencyRelation::Incompatible
    );
    assert_eq!(
        version.dependencies[3].relation,
        DependencyRelation::Embedded
    );

    // Primary verifiable file selection
    let primary = version.primary_file().expect("primary file");
    assert_eq!(primary.role, FileRole::Primary);
    assert!(primary.is_verifiable());
}

#[test]
fn unknown_dependency_and_loader_values_stay_conservative() {
    let dto: ModrinthVersionDto = serde_json::from_value(serde_json::json!({
        "id": "V",
        "project_id": "P",
        "version_number": "1.0",
        "version_type": "mystery-channel",
        "game_versions": [],
        "loaders": ["exotic-loader"],
        "dependencies": [{"dependency_type": "weird"}],
        "files": []
    }))
    .expect("fixture dto");

    let version = normalize_version(dto).expect("normalized version");
    assert_eq!(version.release_channel, ReleaseChannel::Unknown);
    assert!(version.loaders.is_empty());
    assert_eq!(
        version.dependencies[0].relation,
        DependencyRelation::Unknown
    );
}

#[test]
fn deleted_version_is_marked_unavailable() {
    let dto: ModrinthVersionDto = serde_json::from_value(serde_json::json!({
        "id": "V",
        "project_id": "P",
        "version_number": "1.0",
        "version_type": "release",
        "game_versions": ["1.21.1"],
        "loaders": ["fabric"],
        "status": "deleted",
        "files": []
    }))
    .expect("fixture dto");

    let version = normalize_version(dto).expect("normalized version");
    assert!(!version.available);
}

#[test]
fn multiple_verifiable_files_without_primary_are_ambiguous() {
    let dto: ModrinthVersionDto = serde_json::from_value(serde_json::json!({
        "id": "V",
        "project_id": "P",
        "version_number": "1.0",
        "version_type": "release",
        "game_versions": ["1.21.1"],
        "loaders": ["fabric"],
        "files": [
            {"hashes": {"sha1": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}, "url": "https://a.invalid/a.jar", "filename": "a.jar", "primary": false, "size": 1},
            {"hashes": {"sha1": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}, "url": "https://b.invalid/b.jar", "filename": "b.jar", "primary": false, "size": 2}
        ]
    }))
    .expect("fixture dto");

    let version = normalize_version(dto).expect("normalized version");
    assert!(
        version.primary_file().is_none(),
        "ambiguous file sets must not silently select index zero"
    );
}

#[test]
fn malformed_search_response_fails_closed() {
    let result: Result<ModrinthSearchResponseDto, _> =
        serde_json::from_str("{\"hits\": \"not-an-array\"}");
    assert!(result.is_err());

    // Missing required project identity fails during normalization.
    let dto: ModrinthProjectDto = serde_json::from_value(serde_json::json!({
        "id": "",
        "slug": "",
        "title": ""
    }))
    .expect("parses structurally");

    assert!(normalize_project(dto).is_err());
    let _ = ErrorCode::ContentPlanInvalid;
}

#[test]
fn environment_side_semantics_preserved() {
    let dto: ModrinthProjectDto = serde_json::from_value(serde_json::json!({
        "id": "P1",
        "slug": "p",
        "title": "P",
        "client_side": "required",
        "server_side": "unsupported"
    }))
    .expect("fixture dto");

    let project = normalize_project(dto).expect("normalized");
    assert_eq!(project.client_side, EnvironmentSupport::Both);
    assert_eq!(project.server_side, EnvironmentSupport::ServerOnly);
}
