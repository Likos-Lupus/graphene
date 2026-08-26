use graphene_core::CancellationToken;
use graphene_modpack::archive::PackArchiveIndex;
use graphene_modpack::format::{detect_pack_format, modrinth};
use graphene_modpack::model::{
    FileSelection, OptionalSelectionPolicy, PackDiagnosticCode, SeedLayer,
};

fn index_json(files: &str, dependencies: &str) -> String {
    format!(
        r#"{{"formatVersion":1,"game":"minecraft","versionId":"1","name":"Fixture Pack","files":[{files}],"dependencies":{dependencies}}}"#
    )
}

pub use graphene_core::ErrorCode;
use std::collections::BTreeSet;
use std::io::Cursor;

const SHA1_FIXTURE: &str = "0011223344556677889900112233445566778899";
const SHA512_FIXTURE: &str = "0011223344556677889900aabbccddeeff0011223344556677889900aabbccddeeff0011223344556677889900aabbccddeeff0011223344556677889900aabb";

fn build_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = graphene_core::archive::DeterministicZipWriter::new(Cursor::new(Vec::new()));
    for (name, payload) in entries {
        writer.add_entry(name, payload).expect("fixture entry");
    }
    writer.finish().expect("finish");
    writer.into_inner().expect("finalized").into_inner()
}

fn file_entry(path: &str, env_client: &str, host: &str) -> String {
    format!(
        r#"{{"path":"{path}","hashes":{{"sha1":"{SHA1_FIXTURE}","sha512":"{SHA512_FIXTURE}"}},"env":{{"client":"{env_client}","server":"required"}},"downloads":["https://{host}/data/abc/files/{path}"],"fileSize":1024}}"#
    )
}

fn dependencies(minecraft: &str, loader: Option<(&str, &str)>) -> String {
    let mut entries = format!(r#""minecraft":"{minecraft}""#);
    if let Some((key, version)) = loader {
        entries.push_str(&format!(r#", "{key}":"{version}""#));
    }
    format!("{{{entries}}}")
}

fn pack(files: &[String], dependencies: &str) -> Vec<u8> {
    let index = format!(
        r#"{{"formatVersion":1,"game":"minecraft","versionId":"2.1","name":"Fixture Pack","summary":"fixture","files":[{}],"dependencies":{dependencies}}}"#,
        files.join(",")
    );
    build_archive(&[("modrinth.index.json", index.as_bytes())])
}

fn normalize(
    bytes: Vec<u8>,
    policy: &OptionalSelectionPolicy,
) -> Result<graphene_modpack::NormalizedModpack, graphene_modpack::PackError> {
    let mut index = PackArchiveIndex::open(Cursor::new(bytes), &CancellationToken::new())
        .expect("archive opens");
    let detection = detect_pack_format(&mut index, false).expect("detection");
    assert_eq!(detection.format, graphene_modpack::PackFormat::Modrinth);
    modrinth::normalize(&mut index, &detection.root_prefix, policy)
}

fn required_only() -> OptionalSelectionPolicy {
    OptionalSelectionPolicy::RequiredOnly
}

#[test]
fn minimal_valid_pack_normalizes_with_layering() {
    let bytes = build_archive(&[
        (
            "modrinth.index.json",
            index_json(
                "",
                &dependencies("1.21.1", Some(("fabric-loader", "0.16.9"))),
            )
            .as_bytes(),
        ),
        ("overrides/config/mod.cfg", b"a=1\n"),
        ("overrides/options.txt", b"music:0.2\n"),
        ("client-overrides/options.txt", b"music:0.9\n"),
        ("server-overrides/server.properties", b"online-mode=true"),
        ("overrides/mods/local.jar", b"PK\x03\x04local"),
    ]);

    let normalized = normalize(bytes, &required_only()).expect("normalization");
    assert_eq!(normalized.metadata().name(), "Fixture Pack");
    assert_eq!(normalized.runtime().minecraft_version(), "1.21.1");
    let loader = normalized
        .runtime()
        .primary_loader()
        .expect("fabric loader");
    assert_eq!(loader.version(), "0.16.9");

    // Base overrides first, client overrides after.
    let option_layers: Vec<_> = normalized
        .seed_entries()
        .iter()
        .filter(|entry| entry.destination().as_str() == "options.txt")
        .map(|entry| entry.layer())
        .collect();
    assert_eq!(
        option_layers,
        vec![SeedLayer::base(), SeedLayer::client_override()]
    );

    // Embedded override mod is a seed entry at inspection time; promotion to managed content is a
    // planning decision recorded by the service pipeline.
    assert_eq!(normalized.embedded_mod_count(), 1);

    // Server-only data is skipped with a bounded diagnostic.
    assert!(
        normalized
            .diagnostics()
            .iter()
            .any(|d| d.code() == PackDiagnosticCode::ServerOnlyDataSkipped
                && d.message().contains("server-overrides"))
    );
}

#[test]
fn required_optional_and_unsupported_client_semantics() {
    let deps = dependencies("1.21.1", None);
    let files = [
        file_entry("mods/required.jar", "required", "cdn.modrinth.com"),
        file_entry("mods/optional.jar", "optional", "cdn.modrinth.com"),
        file_entry("mods/serverside.jar", "unsupported", "cdn.modrinth.com"),
    ];
    let bytes = pack(&files, &deps);

    let strict = normalize(bytes.clone(), &required_only()).expect("strict");
    assert_eq!(strict.managed_files().len(), 1);
    assert_eq!(
        strict.managed_files()[0].destination().as_str(),
        "mods/required.jar"
    );
    assert!(matches!(
        strict.managed_files()[0].selection(),
        FileSelection::Required
    ));
    assert_eq!(strict.optional_choices().len(), 1);
    assert!(
        strict
            .diagnostics()
            .iter()
            .any(|d| d.code() == PackDiagnosticCode::OptionalFileNotSelected)
    );
    assert!(
        strict
            .diagnostics()
            .iter()
            .any(|d| d.code() == PackDiagnosticCode::ServerOnlyDataSkipped
                && d.message().contains("server-only"))
    );

    let all = normalize(bytes.clone(), &OptionalSelectionPolicy::IncludeAllOptional)
        .expect("include-all");
    assert_eq!(all.managed_files().len(), 2);

    let mut explicit = BTreeSet::new();
    explicit.insert("mrpack:mods/optional.jar".to_owned());
    let chosen = normalize(bytes, &OptionalSelectionPolicy::Explicit(explicit)).expect("explicit");
    assert_eq!(chosen.managed_files().len(), 2);
    assert!(matches!(
        chosen
            .managed_files()
            .iter()
            .find(|file| file.destination().as_str() == "mods/required.jar")
            .expect("required file remains selected")
            .selection(),
        FileSelection::Required
    ));
    assert!(matches!(
        chosen
            .managed_files()
            .iter()
            .find(|file| file.destination().as_str() == "mods/optional.jar")
            .expect("explicit optional file is selected")
            .selection(),
        FileSelection::Optional { .. }
    ));
}

#[test]
fn missing_or_malformed_mandatory_hashes_fail() {
    let deps = dependencies("1.21.1", None);
    let no_sha512 = format!(
        r#"{{"path":"mods/a.jar","hashes":{{"sha1":"{SHA1_FIXTURE}"}},"downloads":["https://cdn.modrinth.com/a.jar"],"fileSize":10}}"#
    );
    let bytes = pack(&[no_sha512], &deps);
    let error = normalize(bytes, &required_only()).expect_err("missing sha512");
    assert_eq!(error.code(), ErrorCode::PackManifestInvalid);

    let bad_sha1 = format!(
        r#"{{"path":"mods/a.jar","hashes":{{"sha1":"zz","sha512":"{SHA512_FIXTURE}"}},"downloads":["https://cdn.modrinth.com/a.jar"],"fileSize":10}}"#
    );
    let bytes = pack(&[bad_sha1], &deps);
    let error = normalize(bytes, &required_only()).expect_err("bad sha1");
    assert_eq!(error.code(), ErrorCode::PackManifestInvalid);
}

#[test]
fn untrusted_download_host_fails_closed() {
    let deps = dependencies("1.21.1", None);
    let evil = file_entry("mods/a.jar", "required", "evil.example.net");
    let bytes = pack(&[evil], &deps);
    let error = normalize(bytes, &required_only()).expect_err("unsafe host");
    assert_eq!(error.code(), ErrorCode::PackArtifactSourceUnsafe);
}

#[test]
fn unsafe_destination_paths_are_rejected() {
    let deps = dependencies("1.21.1", None);
    let traversal = file_entry("../escape.txt", "required", "cdn.modrinth.com");
    let bytes = pack(&[traversal], &deps);
    let error = normalize(bytes, &required_only()).expect_err("traversal");
    assert_eq!(error.code(), ErrorCode::PackPathInvalid);
}

#[test]
fn duplicate_destinations_fail_validation() {
    let deps = dependencies("1.21.1", None);
    let files = [
        file_entry("mods/dup.jar", "required", "cdn.modrinth.com"),
        file_entry("MODS/DUP.JAR", "required", "cdn.modrinth.com"),
    ];
    let bytes = pack(&files, &deps);
    let error = normalize(bytes, &required_only()).expect_err("case collision");
    assert_eq!(error.code(), ErrorCode::PackPathInvalid);
}

#[test]
fn unsupported_and_unknown_runtime_dependencies_fail() {
    let quilt_deps = dependencies("1.21.1", Some(("quilt-loader", "0.26.0")));
    let bytes = pack(&[], &quilt_deps);
    let error = normalize(bytes, &required_only()).expect_err("quilt");
    assert_eq!(error.code(), ErrorCode::PackRuntimeUnsupported);

    let unknown_deps = r#"{"minecraft":"1.21.1","sodium-extra-runtime":"9"}"#;
    let bytes = pack(&[], unknown_deps);
    let error = normalize(bytes, &required_only()).expect_err("unknown dependency");
    assert_eq!(error.code(), ErrorCode::PackManifestInvalid);
}

#[test]
fn forge_and_neoforge_loaders_normalize_exactly() {
    for (key, expected) in [("forge", "47.3.0"), ("neoforge", "20.6.119")] {
        let deps = dependencies("1.20.1", Some((key, expected)));
        let bytes = pack(&[], &deps);
        let normalized = normalize(bytes, &required_only()).expect(key);
        assert_eq!(
            normalized
                .runtime()
                .primary_loader()
                .expect("loader")
                .version(),
            expected
        );
    }
}

#[test]
fn multiple_primary_loaders_fail() {
    let deps = r#"{"minecraft":"1.21.1","fabric-loader":"0.16.9","forge":"47.3.0"}"#;
    let bytes = pack(&[], deps);
    let error = normalize(bytes, &required_only()).expect_err("two loaders");
    assert_eq!(error.code(), ErrorCode::PackRuntimeUnsupported);
}

#[test]
fn wrapper_root_packs_normalize_through_the_prefix() {
    let index = format!(
        r#"{{"formatVersion":1,"game":"minecraft","versionId":"1","name":"Wrapped","files":[],"dependencies":{}}}"#,
        dependencies("1.21.1", None)
    );
    let bytes = build_archive(&[
        ("Wrapped/modrinth.index.json", index.as_bytes()),
        ("Wrapped/overrides/config.toml", b"x=1"),
    ]);
    let normalized = normalize(bytes, &required_only()).expect("wrapped");
    let destination = normalized.seed_entries()[0].destination();
    assert_eq!(destination.as_str(), "config.toml");
}
