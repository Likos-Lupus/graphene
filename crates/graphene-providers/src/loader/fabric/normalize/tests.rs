use super::*;
use crate::loader::fabric::dto::ProfileDto;

fn profile(json: &str) -> ProfileDto {
    serde_json::from_str(json).expect("profile json")
}

#[test]
fn fabric_profile_normalizes_without_preparation_processors() {
    let dto = profile(
        r#"{
      "inheritsFrom":"1.21.1",
      "mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient",
      "arguments":{"jvm":["-Dfabric=true"],"game":["--fabric"]},
      "libraries":[{
        "name":"net.fabricmc:fabric-loader:0.16.14",
        "url":"https://maven.fabricmc.net/",
        "sha1":"d83ef5c6f32a695513e248ff8eee0996c1b31125",
        "sha256":"221f52fb6a8635bd7e5958e55f6cd9dd9718f5f9f6cbbbc073ead12efce80c45",
        "size":12
      }]
    }"#,
    );
    let resolved = normalize_profile(
        dto,
        &MinecraftVersionId::new("1.21.1").expect("minecraft"),
        &LoaderVersion::new("0.16.14").expect("loader"),
        &FabricProviderConfig::default(),
    )
    .expect("normalize");

    assert_eq!(resolved.kind, LoaderKind::Fabric);
    assert_eq!(resolved.support, LoaderSupport::Supported);
    assert_eq!(
        resolved.preparation,
        graphene_minecraft::ComponentPreparationRecipe::default()
    );
    assert_eq!(resolved.patch.libraries.len(), 1);
    assert_eq!(resolved.patch.jvm_args.len(), 1);
    assert_eq!(resolved.patch.game_args.len(), 1);
    assert_eq!(
        resolved.patch.main_class.as_deref(),
        Some("net.fabricmc.loader.impl.launch.knot.KnotClient")
    );

    assert!(
        resolved
            .patch
            .libraries
            .iter()
            .all(|library| !library.coordinate.group.contains("fabric-api")),
        "Fabric API must not be auto-installed by loader normalization"
    );
}

#[test]
fn fabric_profile_rejects_base_mismatch_missing_hash_and_unsafe_repository() {
    let mismatch = profile(
        r#"{
      "inheritsFrom":"1.20.1",
      "mainClass":"KnotClient",
      "libraries":[]
    }"#,
    );

    assert_eq!(
        normalize_profile(
            mismatch,
            &MinecraftVersionId::new("1.21.1").expect("minecraft"),
            &LoaderVersion::new("0.16.14").expect("loader"),
            &FabricProviderConfig::default(),
        )
        .expect_err("base mismatch")
        .code,
        ErrorCode::LoaderProfileInvalid
    );

    let no_hash = profile(
        r#"{
      "inheritsFrom":"1.21.1",
      "mainClass":"KnotClient",
      "libraries":[{"name":"org.example:demo:1","url":"https://example.invalid/"}]
    }"#,
    );
    assert_eq!(
        normalize_profile(
            no_hash,
            &MinecraftVersionId::new("1.21.1").expect("minecraft"),
            &LoaderVersion::new("0.16.14").expect("loader"),
            &FabricProviderConfig::default(),
        )
        .expect_err("missing provider integrity")
        .code,
        ErrorCode::LoaderArtifactUnverifiable
    );

    let unsafe_repo = profile(
        r#"{
      "inheritsFrom":"1.21.1",
      "mainClass":"KnotClient",
      "libraries":[{
        "name":"org.example:demo:1",
        "url":"http://127.0.0.1/",
        "sha1":"d83ef5c6f32a695513e248ff8eee0996c1b31125"
      }]
    }"#,
    );
    assert_eq!(
        normalize_profile(
            unsafe_repo,
            &MinecraftVersionId::new("1.21.1").expect("minecraft"),
            &LoaderVersion::new("0.16.14").expect("loader"),
            &FabricProviderConfig::default(),
        )
        .expect_err("unsafe repo")
        .code,
        ErrorCode::ConfigInvalid
    );
}

#[test]
fn fabric_profile_rejects_non_string_argument_data() {
    let dto = profile(
        r#"{
      "inheritsFrom":"1.21.1",
      "mainClass":"KnotClient",
      "arguments":{"game":[{"rules":[],"value":"--unsupported-shape"}]},
      "libraries":[]
    }"#,
    );
    assert_eq!(
        normalize_profile(
            dto,
            &MinecraftVersionId::new("1.21.1").expect("minecraft"),
            &LoaderVersion::new("0.16.14").expect("loader"),
            &FabricProviderConfig::default(),
        )
        .expect_err("unsupported argument")
        .code,
        ErrorCode::LoaderProfileInvalid
    );
}
