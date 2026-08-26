use graphene_core::archive::DeterministicZipWriter;
use graphene_core::{CancellationToken, ErrorCode};
use graphene_minecraft::LoaderKind;
use graphene_modpack::PackFormat;
use graphene_modpack::archive::PackArchiveIndex;
use graphene_modpack::format::{detect_pack_format, multimc};
use std::io::Cursor;

fn build_archive(entries: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    let mut writer = DeterministicZipWriter::new(Cursor::new(Vec::new()));
    for (name, payload) in entries {
        writer.add_entry(&name, &payload).expect("fixture entry");
    }
    writer.finish().expect("finish");
    writer.into_inner().expect("finalized").into_inner()
}

fn component(uid: &str, version: &str) -> String {
    format!(r#"{{"uid":"{uid}","version":"{version}"}}"#)
}

fn manifest(components: &[String]) -> String {
    format!(
        r#"{{"formatVersion":1,"components":[{}]}}"#,
        components.join(",")
    )
}

fn pack(
    root: &str,
    content_dir: &str,
    components: &[String],
    mut extra: Vec<(String, Vec<u8>)>,
) -> Vec<u8> {
    let manifest = manifest(components);
    let mut entries = vec![
        (format!("{root}mmc-pack.json"), manifest.into_bytes()),
        (format!("{root}{content_dir}/"), Vec::new()),
    ];
    entries.append(&mut extra);
    build_archive(entries)
}

fn normalize(
    bytes: Vec<u8>,
) -> Result<multimc::NormalizedMultiMcPack, graphene_modpack::PackError> {
    let mut index = PackArchiveIndex::open(Cursor::new(bytes), &CancellationToken::new())
        .expect("archive opens");
    let detection = detect_pack_format(&mut index, false).expect("detection");
    assert_eq!(detection.format, PackFormat::PrismMultiMc);
    multimc::normalize(&mut index, &detection.root_prefix)
}

#[test]
fn standard_runtime_components_map_to_exact_graphene_requirements() {
    for (uid, version, expected_kind) in [
        (FABRIC_UID, "0.16.9", LoaderKind::Fabric),
        (FORGE_UID, "47.3.0", LoaderKind::Forge),
        (NEOFORGE_UID, "20.6.119", LoaderKind::NeoForge),
    ] {
        let components = vec![
            component("org.lwjgl3", "3.3.3"),
            component("net.minecraft", "1.20.1"),
            component(uid, version),
        ];
        let normalized = normalize(pack("", ".minecraft", &components, Vec::new())).expect(uid);
        assert_eq!(normalized.runtime().minecraft_version(), "1.20.1");
        let loader = normalized
            .runtime()
            .primary_loader()
            .expect("primary loader");
        assert_eq!(loader.kind(), expected_kind);
        assert_eq!(loader.version(), version);
    }
}

const FABRIC_UID: &str = "net.fabricmc.fabric-loader";
const FORGE_UID: &str = "net.minecraftforge";
const NEOFORGE_UID: &str = "net.neoforged";

#[test]
fn content_is_split_into_embedded_mods_and_seed_files() {
    let components = vec![component("net.minecraft", "1.21.1")];
    let bytes = pack(
        "",
        ".minecraft",
        &components,
        vec![
            (
                "instance.cfg".to_owned(),
                b"name=Imported Fixture\nJavaPath=/secret/java\nMaxMemAlloc=8192\n".to_vec(),
            ),
            ("accounts.json".to_owned(), b"secret".to_vec()),
            ("java/runtime/bin/java".to_owned(), b"foreign java".to_vec()),
            (
                "libraries/example/runtime.jar".to_owned(),
                b"foreign library".to_vec(),
            ),
            (
                "settings/launcher.cfg".to_owned(),
                b"foreign settings".to_vec(),
            ),
            (
                ".minecraft/libraries/example.jar".to_owned(),
                b"cached library".to_vec(),
            ),
            (
                ".minecraft/accounts.json".to_owned(),
                b"embedded account".to_vec(),
            ),
            (
                ".minecraft/java/runtime/bin/java".to_owned(),
                b"embedded java".to_vec(),
            ),
            (
                ".minecraft/settings/launcher.cfg".to_owned(),
                b"embedded settings".to_vec(),
            ),
            (
                ".minecraft/mods/managed.jar".to_owned(),
                b"embedded mod".to_vec(),
            ),
            (
                ".minecraft/config/example.toml".to_owned(),
                b"enabled=true".to_vec(),
            ),
            (".minecraft/options.txt".to_owned(), b"music:0.2".to_vec()),
        ],
    );

    let normalized = normalize(bytes).expect("normalization");
    assert_eq!(normalized.metadata().name(), "Imported Fixture");
    assert_eq!(normalized.embedded_mods().len(), 1);
    assert_eq!(
        normalized.embedded_mods()[0].destination().as_str(),
        "mods/managed.jar"
    );
    let seeds: Vec<_> = normalized
        .seed_entries()
        .iter()
        .map(|entry| entry.destination().as_str())
        .collect();
    assert_eq!(seeds, vec!["config/example.toml", "options.txt"]);
    assert!(
        normalized
            .seed_entries()
            .iter()
            .all(|entry| !entry.archive_entry().contains("secret"))
    );
}

#[test]
fn historical_content_root_and_wrapper_are_supported() {
    let components = vec![component("net.minecraft", "1.20.4")];
    let bytes = pack(
        "Wrapped/",
        "minecraft",
        &components,
        vec![
            (
                "Wrapped/instance.cfg".to_owned(),
                b"name=Wrapped Pack\n".to_vec(),
            ),
            ("Wrapped/minecraft/config/a.cfg".to_owned(), b"a=1".to_vec()),
        ],
    );
    let normalized = normalize(bytes).expect("wrapped historical export");
    assert_eq!(normalized.metadata().name(), "Wrapped Pack");
    assert_eq!(
        normalized.seed_entries()[0].destination().as_str(),
        "config/a.cfg"
    );
}

#[test]
fn runtime_composition_and_versions_fail_closed() {
    let duplicate_minecraft = vec![
        component("net.minecraft", "1.20.1"),
        component("net.minecraft", "1.20.2"),
    ];
    let error = normalize(pack("", ".minecraft", &duplicate_minecraft, Vec::new()))
        .expect_err("duplicate Minecraft");
    assert_eq!(error.code(), ErrorCode::PackManifestInvalid);

    let multiple_loaders = vec![
        component("net.minecraft", "1.20.1"),
        component(FABRIC_UID, "0.16.9"),
        component(FORGE_UID, "47.3.0"),
    ];
    let error = normalize(pack("", ".minecraft", &multiple_loaders, Vec::new()))
        .expect_err("multiple loaders");
    assert_eq!(error.code(), ErrorCode::PackRuntimeUnsupported);

    let moving_version = vec![component("net.minecraft", "latest")];
    let error =
        normalize(pack("", ".minecraft", &moving_version, Vec::new())).expect_err("moving version");
    assert_eq!(error.code(), ErrorCode::PackManifestInvalid);

    let missing_version = r#"{"formatVersion":1,"components":[{"uid":"net.minecraft"}]}"#;
    let error = normalize(build_archive(vec![
        (
            "mmc-pack.json".to_owned(),
            missing_version.as_bytes().to_vec(),
        ),
        (".minecraft/".to_owned(), Vec::new()),
    ]))
    .expect_err("missing version");
    assert_eq!(error.code(), ErrorCode::PackManifestInvalid);
}

#[test]
fn custom_components_nonredundant_patches_and_jarmods_are_rejected() {
    let custom = vec![
        component("net.minecraft", "1.21.1"),
        component("custom.launcher.patch", "1.0.0"),
    ];
    let error =
        normalize(pack("", ".minecraft", &custom, Vec::new())).expect_err("custom component");
    assert_eq!(error.code(), ErrorCode::PackRuntimeUnsupported);
    assert!(error.message().contains("custom component"));

    let components = vec![component("net.minecraft", "1.21.1")];
    let patch = br#"{"formatVersion":1,"uid":"net.minecraft","version":"1.21.1","libraries":[{"name":"evil:replacement:1"}]}"#;
    let error = normalize(pack(
        "",
        ".minecraft",
        &components,
        vec![("patches/net.minecraft.json".to_owned(), patch.to_vec())],
    ))
    .expect_err("launch patch");
    assert_eq!(error.code(), ErrorCode::PackRuntimeUnsupported);
    assert!(error.message().contains("patch"));

    let error = normalize(pack(
        "",
        ".minecraft",
        &components,
        vec![("jarmods/client.jar".to_owned(), b"jar mutation".to_vec())],
    ))
    .expect_err("jarmods");
    assert_eq!(error.code(), ErrorCode::PackRuntimeUnsupported);
    assert!(error.message().contains("jarmods"));

    let error = normalize(pack(
        "",
        ".minecraft",
        &components,
        vec![(
            ".minecraft/jarmods/client.jar".to_owned(),
            b"jar mutation".to_vec(),
        )],
    ))
    .expect_err("payload jarmods");
    assert_eq!(error.code(), ErrorCode::PackRuntimeUnsupported);
    assert!(error.message().contains("jarmods"));
}

#[test]
fn identity_only_redundant_patch_is_ignored() {
    let components = vec![component("net.minecraft", "1.21.1")];
    let patch =
        br#"{"formatVersion":1,"uid":"net.minecraft","version":"1.21.1","name":"Minecraft"}"#;
    let normalized = normalize(pack(
        "",
        ".minecraft",
        &components,
        vec![
            ("patches/net.minecraft.json".to_owned(), patch.to_vec()),
            (".minecraft/config/a.cfg".to_owned(), b"a=1".to_vec()),
        ],
    ))
    .expect("redundant patch");
    assert_eq!(normalized.seed_entries().len(), 1);
}

#[test]
fn display_name_is_bounded_and_only_instance_cfg_can_supply_it() {
    let components = vec![component("net.minecraft", "1.21.1")];
    let no_cfg = normalize(pack(
        "",
        ".minecraft",
        &components,
        vec![("pack.name".to_owned(), b"Untrusted Name".to_vec())],
    ))
    .expect("fallback name");
    assert_eq!(no_cfg.metadata().name(), "Prism/MultiMC instance");

    let long_name = "x".repeat(257);
    let error = normalize(pack(
        "",
        ".minecraft",
        &components,
        vec![(
            "instance.cfg".to_owned(),
            format!("name={long_name}\n").into_bytes(),
        )],
    ))
    .expect_err("unbounded name");
    assert_eq!(error.code(), ErrorCode::PackManifestInvalid);
}
