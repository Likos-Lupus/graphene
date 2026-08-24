use crate::content::curseforge::config::CurseForgeProviderConfig;
use crate::content::curseforge::dto::{CurseForgeFileDto, CurseForgeSingleFileResponseDto};
use crate::content::curseforge::normalize::{
    normalize_file, normalize_release_channel, provider_id,
};
use graphene_content::{
    ContentProvider as _, DependencyRelation, EnvironmentSupport, ExactMatchStatus, FileMatchItem,
    FileMatchRequest, FileRole, ReleaseChannel,
};
use graphene_core::{ErrorCode, Result, SensitiveString};

fn file_fixture(json: serde_json::Value) -> CurseForgeFileDto {
    let wrapper: CurseForgeSingleFileResponseDto =
        serde_json::from_value(serde_json::json!({ "data": json })).expect("fixture dto");
    wrapper.data
}

fn complete_file_json() -> serde_json::Value {
    serde_json::json!({
        "id": 777u32,
        "gameId": 432u32,
        "modId": 555u32,
        "isAvailable": true,
        "displayName": "Awesome Mod 1.2.3",
        "fileName": "awesome-mod-1.2.3.jar",
        "releaseType": 1u32,
        "fileStatus": 4u32,
        "hashes": [{"value": "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12", "algo": 1u32}],
        "fileDate": "2024-06-01T00:00:00Z",
        "fileLength": 2048u64,
        "downloadCount": 10u64,
        "downloadUrl": "https://mediafile.forgecdn.net/files/awesome.jar",
        "gameVersions": ["1.21.1", "Fabric"],
        "dependencies": [
            {"modId": 111u32, "relationType": 3u32},
            {"modId": 222u32, "relationType": 2u32},
            {"modId": 333u32, "relationType": 5u32},
            {"modId": 444u32, "relationType": 1u32}
        ],
        "packageFingerprint": 305419896u32
    })
}

#[test]
fn release_type_normalization_matches_provider_semantics() {
    assert_eq!(normalize_release_channel(1), ReleaseChannel::Release);
    assert_eq!(normalize_release_channel(2), ReleaseChannel::Beta);
    assert_eq!(normalize_release_channel(3), ReleaseChannel::Alpha);
    assert_eq!(normalize_release_channel(99), ReleaseChannel::Unknown);
}

#[test]
fn file_normalizes_identity_hashes_and_dependencies() {
    let dto = file_fixture(complete_file_json());
    let pid = provider_id();
    let (version, file) = normalize_file(dto, &pid).expect("normalized");

    // Numeric provider IDs remain opaque strings in Graphene identity.
    assert_eq!(version.version_ref.project_id, "555");
    assert_eq!(version.version_ref.version_id, "777");
    assert_eq!(version.version_ref.provider.as_str(), "curseforge");
    assert_eq!(version.release_channel, ReleaseChannel::Release);
    assert!(
        version
            .loaders
            .contains(&graphene_minecraft::LoaderKind::Fabric)
    );
    assert!(version.game_versions.contains(&"1.21.1".to_string()));

    assert_eq!(
        version.dependencies[0].relation,
        DependencyRelation::Required
    );
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

    assert_eq!(file.role, FileRole::Primary);
    assert!(file.is_verifiable());
    assert_eq!(file.murmur2_fingerprint, Some(305_419_896));
}

#[test]
fn file_without_strong_digest_is_not_installable() {
    let mut json = complete_file_json();
    json["hashes"] = serde_json::json!([
        {"value": "d41d8cd98f00b204e9800998ecf8427e", "algo": 2u32}
    ]);
    let dto = file_fixture(json);
    let (_, file) = normalize_file(dto, &provider_id()).expect("normalized");

    assert!(
        !file.is_verifiable(),
        "MD5 alone must never satisfy artifact integrity"
    );
}

#[test]
fn file_without_download_url_is_not_installable() {
    let mut json = complete_file_json();
    json["downloadUrl"] = serde_json::Value::Null;
    let dto = file_fixture(json);
    let (_, file) = normalize_file(dto, &provider_id()).expect("normalized");

    assert!(!file.is_verifiable());
}

#[test]
fn unavailable_or_unapproved_file_is_marked_unavailable() {
    let mut json = complete_file_json();
    json["isAvailable"] = serde_json::Value::Bool(false);
    let dto = file_fixture(json);
    let (version, file) = normalize_file(dto, &provider_id()).expect("normalized");
    assert!(!file.available);
    assert!(!version.available);

    let mut json = complete_file_json();
    json["fileStatus"] = serde_json::json!(1); // not approved
    let dto = file_fixture(json);
    let (_, file) = normalize_file(dto, &provider_id()).expect("normalized");
    assert!(!file.available);

    // Environment defaults remain conservative for client-side evaluation.
    assert_eq!(EnvironmentSupport::default(), EnvironmentSupport::Both);
}

#[test]
fn missing_api_key_produces_typed_unavailable_error() {
    let network = test_network_client();
    let config = CurseForgeProviderConfig::production(None).expect("valid config");
    let provider =
        crate::content::curseforge::provider::CurseForgeContentProvider::new(network, config)
            .expect("provider without key constructs");

    // The port surface must expose the capability state, and the adapter's key gate fails typed.
    assert!(provider.capabilities().requires_credentials);
    let error = provider.check_key().expect_err("missing key");
    assert_eq!(error.code, ErrorCode::ContentProviderUnavailable);

    // Unmatched recognition requests degrade to explicit unmatched results.
    let request = FileMatchRequest {
        items: vec![FileMatchItem::new(None, None, Some(42), 100)],
    };
    let operation = test_operation();
    let matches = futures_now(provider.match_files(&request, &operation)).expect("matches");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].status, ExactMatchStatus::Unmatched);

    let _ = Result::<()>::Ok(());
}

fn futures_now<T>(fut: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
        .block_on(fut)
}

fn test_network_client() -> graphene_network::NetworkClient {
    graphene_network::NetworkClient::new(graphene_network::NetworkConfig::default())
        .expect("default network client")
}

fn test_operation() -> graphene_core::OperationController {
    let registry = graphene_core::OperationRegistry::new(16).expect("valid registry");
    registry.create("content-test")
}

#[test]
fn api_key_is_redacted_in_debug_output() {
    let config =
        CurseForgeProviderConfig::production(Some(SensitiveString::new("super-secret-key")))
            .expect("valid config");

    let debug = format!("{config:?}");
    assert!(!debug.contains("super-secret-key"));
}
