use graphene_core::CancellationToken;
use graphene_core::archive::DeterministicZipWriter;
use graphene_modpack::PackFormat;
use graphene_modpack::archive::PackArchiveIndex;
use graphene_modpack::format::detect_pack_format;
use std::io::Cursor;

fn build_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = DeterministicZipWriter::new(Cursor::new(Vec::new()));
    for (name, payload) in entries {
        writer.add_entry(name, payload).expect("fixture entry");
    }
    writer.finish().expect("finish");
    writer.into_inner().expect("finalized").into_inner()
}

fn open(bytes: Vec<u8>) -> PackArchiveIndex<Cursor<Vec<u8>>> {
    PackArchiveIndex::open(Cursor::new(bytes), &CancellationToken::new()).expect("valid archive")
}

const MODRINTH_INDEX: &str = r#"{"formatVersion":1,"game":"minecraft","versionId":"1.0.0","name":"Demo","files":[],"dependencies":{"minecraft":"1.21.1","fabric-loader":"0.16.9"}}"#;
const CURSEFORGE_MANIFEST: &str = r#"{"minecraft":{"version":"1.21.1","modLoaders":[{"id":"fabric-0.16.9","primary":true}]},"manifestType":"minecraftModpack","manifestVersion":1,"name":"CF Demo","version":"1.0","author":"someone","files":[]}"#;
const MMC_PACK: &str = r#"{"formatVersion":1,"components":[{"important":true,"cachedName":"Minecraft","uid":"net.minecraft","version":"1.20.1"}]}"#;
const GRAPHENE_PACK: &str = r#"{"schema_version":1,"pack":{"name":"G Demo"},"runtime":{"minecraft_version":"1.21.1"},"managed_files":[],"seed_files":[]}"#;

#[test]
fn detects_modrinth_at_archive_root() {
    let bytes = build_archive(&[
        ("modrinth.index.json", MODRINTH_INDEX.as_bytes()),
        ("overrides/config.toml", b"x = 1"),
    ]);
    let mut index = open(bytes);
    let detection = detect_pack_format(&mut index, false).expect("detection");
    assert_eq!(detection.format, PackFormat::Modrinth);
    assert_eq!(detection.root_prefix, "");
}

#[test]
fn detects_curseforge_manifest() {
    let bytes = build_archive(&[
        ("manifest.json", CURSEFORGE_MANIFEST.as_bytes()),
        ("overrides/mods/readme.txt", b"note"),
    ]);
    let mut index = open(bytes);
    let detection = detect_pack_format(&mut index, false).expect("detection");
    assert_eq!(detection.format, PackFormat::CurseForge);
}

#[test]
fn detects_multimc_instance_layout() {
    let bytes = build_archive(&[
        ("mmc-pack.json", MMC_PACK.as_bytes()),
        (".minecraft/", b""),
        (".minecraft/options.txt", b"soundCategory:music:0.1"),
    ]);
    let mut index = open(bytes);
    let detection = detect_pack_format(&mut index, false).expect("detection");
    assert_eq!(detection.format, PackFormat::PrismMultiMc);
}

#[test]
fn detects_graphene_pack() {
    let bytes = build_archive(&[
        ("graphene.pack.json", GRAPHENE_PACK.as_bytes()),
        ("seed/config/app.cfg", b"value=1"),
    ]);
    let mut index = open(bytes);
    let detection = detect_pack_format(&mut index, false).expect("detection");
    assert_eq!(detection.format, PackFormat::Graphene);
}

#[test]
fn strips_a_single_unambiguous_wrapper() {
    let bytes = build_archive(&[
        ("MyPack/modrinth.index.json", MODRINTH_INDEX.as_bytes()),
        ("MyPack/overrides/a.txt", b"a"),
    ]);
    let mut index = open(bytes);
    let detection = detect_pack_format(&mut index, false).expect("detection");
    assert_eq!(detection.format, PackFormat::Modrinth);
    assert_eq!(detection.root_prefix, "MyPack/");
}

#[test]
fn conflicting_format_manifests_are_ambiguous() {
    let bytes = build_archive(&[
        ("modrinth.index.json", MODRINTH_INDEX.as_bytes()),
        ("manifest.json", CURSEFORGE_MANIFEST.as_bytes()),
    ]);
    let mut index = open(bytes);
    let error = detect_pack_format(&mut index, false).expect_err("ambiguous");
    assert_eq!(error.code(), graphene_core::ErrorCode::PackFormatAmbiguous);
}

#[test]
fn unrecognized_archive_requires_explicit_generic_mode() {
    let bytes = build_archive(&[("stuff/readme.txt", b"hello")]);
    let mut index = open(bytes.clone());
    let error = detect_pack_format(&mut index, false).expect_err("unknown format");
    assert_eq!(error.code(), graphene_core::ErrorCode::PackFormatUnknown);

    let mut index = open(bytes);
    let detection = detect_pack_format(&mut index, true).expect("explicit generic allowed");
    assert_eq!(detection.format, PackFormat::Generic);
}

#[test]
fn malformed_recognized_manifest_fails_instead_of_falling_back() {
    let bytes = build_archive(&[("modrinth.index.json", b"{not json")]);
    let mut index = open(bytes);
    let error = detect_pack_format(&mut index, true).expect_err("malformed manifest");
    assert_eq!(error.code(), graphene_core::ErrorCode::PackManifestInvalid);
}

#[test]
fn unsupported_modrinth_format_version_is_reported() {
    const FUTURE: &str = r#"{"formatVersion":99,"game":"minecraft"}"#;
    let bytes = build_archive(&[("modrinth.index.json", FUTURE.as_bytes())]);
    let mut index = open(bytes);
    let result = detect_pack_format(&mut index, true);
    assert!(
        result.is_ok(),
        "future format versions are recognized but rejected during normalization"
    );
    assert_eq!(result.expect("recognized").format, PackFormat::Modrinth);
}

#[test]
fn encrypted_archives_fail_indexing() {
    // Reuse the shared malicious fixtures from the core archive suite.
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../graphene-core/tests/fixtures/archive/encrypted_flag.zip"
    );
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let error = PackArchiveIndex::open(Cursor::new(bytes), &CancellationToken::new())
        .err()
        .expect("encrypted archive must be rejected");
    assert_eq!(error.code(), graphene_core::ErrorCode::PackArchiveInvalid);
}
