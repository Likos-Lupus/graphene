use super::*;
use graphene_core::{ArtifactIntegrity, ArtifactKind, ErrorCode, Sha1Digest};

fn metadata_artifact(path: &str) -> ResolvedArtifact {
    let digest: Sha1Digest = "0000000000000000000000000000000000000000"
        .parse()
        .expect("digest");
    let artifact = artifact_for(
        ArtifactKind::Metadata,
        "https://example.invalid/index.json".to_owned(),
        ArtifactIntegrity::none().with_sha1(digest),
        Some(2),
        false,
    )
    .expect("artifact");
    ResolvedArtifact {
        artifact,
        relative_path: ManagedPath::new(path).expect("managed path"),
    }
}

#[test]
fn manifest_normalization_preserves_explicit_version_type() {
    let bytes = br#"{
      "latest":{"release":"1.21.1","snapshot":"24w33a"},
      "versions":[{"id":"1.21.1","type":"release","url":"https://example.invalid/v.json","sha1":"0000000000000000000000000000000000000000","releaseTime":"2024-08-08T00:00:00Z","complianceLevel":1}]
    }"#;
    let manifest = normalize_manifest(bytes, &MojangProviderConfig::default()).expect("manifest");
    assert_eq!(manifest.versions.len(), 1);
    assert_eq!(manifest.versions[0].id.as_str(), "1.21.1");
    assert_eq!(
        manifest.versions[0].version_type,
        MinecraftVersionType::Release
    );
}

#[test]
fn unknown_manifest_version_is_structured() {
    let bytes = br#"{"latest":{"release":"x","snapshot":"x"},"versions":[]}"#;
    let manifest = normalize_manifest(bytes, &MojangProviderConfig::default()).expect("manifest");
    let error = manifest
        .select(&MinecraftVersionId::new("missing").expect("id"))
        .expect_err("missing");
    assert_eq!(error.code, ErrorCode::MinecraftVersionNotFound);
}

#[test]
fn modern_arguments_libraries_and_java_are_normalized() {
    let bytes = br#"{
      "id":"1.21.1",
      "type":"release",
      "mainClass":"net.minecraft.client.main.Main",
      "arguments":{
        "jvm":["-Xmx1G",{"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],"value":["-Ddemo=true","${classpath}"]}],
        "game":["--username","${auth_player_name}"]
      },
      "libraries":[{
        "name":"org.example:demo:1.0",
        "downloads":{"artifact":{"sha1":"0000000000000000000000000000000000000000","size":1,"url":"https://example.invalid/demo.jar","path":"org/example/demo/1.0/demo-1.0.jar"}}
      }],
      "assets":"fixture",
      "assetIndex":{"id":"fixture","sha1":"0000000000000000000000000000000000000000","size":2,"url":"https://example.invalid/assets.json"},
      "javaVersion":{"component":"java-runtime-gamma","majorVersion":21}
    }"#;
    let metadata =
        normalize_version_metadata(bytes, &MojangProviderConfig::default()).expect("metadata");
    assert_eq!(metadata.id.as_str(), "1.21.1");
    assert_eq!(metadata.jvm_args.len(), 2);
    assert_eq!(metadata.game_args.len(), 2);
    assert_eq!(metadata.libraries.len(), 1);
    assert_eq!(
        metadata.libraries[0]
            .coordinate
            .repository_path()
            .expect("repository path")
            .as_str(),
        "org/example/demo/1.0/demo-1.0.jar"
    );
    assert_eq!(
        metadata
            .java_requirement
            .expect("java requirement")
            .major_version,
        21
    );
}

#[test]
fn asset_hash_derives_source_and_managed_object_path() {
    let bytes = br#"{"objects":{"sounds/demo.ogg":{"hash":"1234567890abcdef1234567890abcdef12345678","size":7}}}"#;
    let assets = normalize_asset_index(
        "fixture",
        metadata_artifact("shared/assets/indexes/fixture.json"),
        bytes,
        "https://resources.example.invalid",
        &MojangProviderConfig::default(),
    )
    .expect("assets");
    assert_eq!(assets.objects.len(), 1);
    let object = &assets.objects[0];
    assert_eq!(object.logical_name, "sounds/demo.ogg");
    assert_eq!(
        object.artifact.relative_path.as_str(),
        "shared/assets/objects/12/1234567890abcdef1234567890abcdef12345678"
    );
    assert_eq!(
        object.artifact.artifact.sources[0].url(),
        "https://resources.example.invalid/12/1234567890abcdef1234567890abcdef12345678"
    );
}

#[test]
fn bounded_json_normalization_property_inputs_do_not_panic() {
    let mut state = 0x243f_6a88_u32;
    for length in 0..256usize {
        let mut bytes = Vec::with_capacity(length);
        for _ in 0..length {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            bytes.push((state >> 24) as u8);
        }
        let _ = normalize_manifest(&bytes, &MojangProviderConfig::default());
        let _ = normalize_version_metadata(&bytes, &MojangProviderConfig::default());
    }
}

#[test]
fn invalid_asset_hash_uses_asset_index_error_family() {
    let bytes = br#"{"objects":{"bad":{"hash":"not-a-sha1","size":7}}}"#;
    let error = normalize_asset_index(
        "fixture",
        metadata_artifact("shared/assets/indexes/fixture.json"),
        bytes,
        "https://resources.example.invalid",
        &MojangProviderConfig::default(),
    )
    .expect_err("invalid asset digest");
    assert_eq!(error.code, ErrorCode::MinecraftAssetIndexInvalid);
}
