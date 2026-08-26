use graphene_core::archive::DeterministicZipWriter;
use graphene_core::{CancellationToken, ErrorCode};
use graphene_minecraft::LoaderKind;
use graphene_modpack::archive::PackArchiveIndex;
use graphene_modpack::format::generic::{GenericContentRoot, GenericImportOptions, normalize};
use graphene_modpack::model::PackLoaderRequirement;
use std::io::Cursor;

const MODRINTH_INDEX: &str = r#"{"formatVersion":1,"game":"minecraft","versionId":"1","name":"Known","files":[],"dependencies":{"minecraft":"1.21.1"}}"#;

fn build_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = DeterministicZipWriter::new(Cursor::new(Vec::new()));
    for (name, payload) in entries {
        writer.add_entry(name, payload).expect("fixture entry");
    }
    writer.finish().expect("finish fixture");
    writer.into_inner().expect("finalized").into_inner()
}

fn open(bytes: Vec<u8>) -> PackArchiveIndex<Cursor<Vec<u8>>> {
    PackArchiveIndex::open(Cursor::new(bytes), &CancellationToken::new()).expect("valid archive")
}

fn options(root: GenericContentRoot) -> GenericImportOptions {
    GenericImportOptions::new("1.21.1", None, root).expect("valid options")
}

#[test]
fn explicit_runtime_root_and_payload_classification_are_deterministic() {
    let nested = build_archive(&[("inside.txt", b"opaque")]);
    let entries = [
        ("Pack/options.txt", b"music:0.5".as_slice()),
        ("Pack/mods/z.jar", b"z".as_slice()),
        ("Pack/config/client.toml", b"enabled=true".as_slice()),
        ("Pack/mods/a.JAR", b"a".as_slice()),
        ("Pack/backups/nested.zip", nested.as_slice()),
    ];
    let mut index = open(build_archive(&entries));
    let loader = PackLoaderRequirement::new(LoaderKind::Fabric, "0.16.9").expect("loader");
    let options = GenericImportOptions::new(
        "1.21.1",
        Some(loader),
        GenericContentRoot::Directory("Pack".to_owned()),
    )
    .expect("options");

    let parsed = normalize(&mut index, &options).expect("generic import");
    assert_eq!(parsed.content_root_prefix(), "Pack/");
    assert_eq!(parsed.runtime().minecraft_version(), "1.21.1");
    assert_eq!(
        parsed.runtime().primary_loader().expect("loader").version(),
        "0.16.9"
    );
    assert_eq!(
        parsed
            .embedded_mods()
            .iter()
            .map(|entry| entry.destination().as_str())
            .collect::<Vec<_>>(),
        vec!["mods/a.JAR", "mods/z.jar"]
    );
    assert_eq!(
        parsed
            .seed_entries()
            .iter()
            .map(|entry| entry.destination().as_str())
            .collect::<Vec<_>>(),
        vec!["backups/nested.zip", "config/client.toml", "options.txt"]
    );
    let nested_entry = parsed
        .seed_entries()
        .iter()
        .find(|entry| entry.destination().as_str() == "backups/nested.zip")
        .expect("nested archive remains one opaque file");
    assert_eq!(nested_entry.size(), nested.len() as u64);
}

#[test]
fn known_formats_and_malformed_known_manifests_cannot_be_bypassed() {
    let mut known = open(build_archive(&[
        ("Known/modrinth.index.json", MODRINTH_INDEX.as_bytes()),
        ("loose.txt", b"unrelated"),
    ]));
    let error = normalize(
        &mut known,
        &options(GenericContentRoot::Directory("Known".to_owned())),
    )
    .expect_err("recognized nested format");
    assert_eq!(error.code(), ErrorCode::PackSourceInvalid);

    let mut malformed = open(build_archive(&[("modrinth.index.json", b"{not json")]));
    let error = normalize(&mut malformed, &options(GenericContentRoot::ArchiveRoot))
        .expect_err("malformed recognized candidate");
    assert_eq!(error.code(), ErrorCode::PackManifestInvalid);
}

#[test]
fn auto_root_refuses_wrapper_ambiguity_and_explicit_choice_resolves_it() {
    let bytes = build_archive(&[
        ("Wrapper/config/a.toml", b"a"),
        ("Wrapper/mods/a.jar", b"m"),
    ]);
    let mut ambiguous = open(bytes.clone());
    let error = normalize(&mut ambiguous, &options(GenericContentRoot::Auto))
        .expect_err("wrapper could be content or container");
    assert_eq!(error.code(), ErrorCode::PackSelectionRequired);

    let mut explicit = open(bytes);
    let parsed = normalize(
        &mut explicit,
        &options(GenericContentRoot::Directory("Wrapper".to_owned())),
    )
    .expect("explicit root");
    assert_eq!(parsed.embedded_mods().len(), 1);
    assert_eq!(parsed.seed_entries().len(), 1);
}

#[test]
fn traversal_reserved_case_and_prefix_collisions_fail_closed() {
    for (entries, expected) in [
        (
            vec![("../escape.txt", b"x".as_slice())],
            ErrorCode::PackPathInvalid,
        ),
        (
            vec![(".graphene/internal/reserved", b"x".as_slice())],
            ErrorCode::PackPathInvalid,
        ),
        (
            vec![
                ("mods/A.jar", b"a".as_slice()),
                ("MODS/a.JAR", b"b".as_slice()),
            ],
            ErrorCode::PackPathInvalid,
        ),
        (
            vec![
                ("config", b"a".as_slice()),
                ("config/a.toml", b"b".as_slice()),
            ],
            ErrorCode::PackPathInvalid,
        ),
        (
            vec![
                ("Config/a.toml", b"a".as_slice()),
                ("config/b.toml", b"b".as_slice()),
            ],
            ErrorCode::PackPathInvalid,
        ),
        (
            vec![("empty", b"a".as_slice()), ("empty/", b"".as_slice())],
            ErrorCode::PackPathInvalid,
        ),
        (
            vec![("versions/base.jar", b"x".as_slice())],
            ErrorCode::PackPathInvalid,
        ),
    ] {
        let mut index = open(build_archive(&entries));
        let error = normalize(&mut index, &options(GenericContentRoot::ArchiveRoot))
            .expect_err("unsafe destination");
        assert_eq!(error.code(), expected);
    }
}

#[test]
fn runtime_versions_must_be_explicit_and_exact() {
    let error = GenericImportOptions::new("latest", None, GenericContentRoot::ArchiveRoot)
        .expect_err("moving Minecraft alias");
    assert_eq!(error.code(), ErrorCode::PackRuntimeUnsupported);

    let loader = PackLoaderRequirement::new(LoaderKind::Forge, "recommended").expect("shape");
    let error = GenericImportOptions::new("1.20.1", Some(loader), GenericContentRoot::ArchiveRoot)
        .expect_err("moving loader alias");
    assert_eq!(error.code(), ErrorCode::PackRuntimeUnsupported);

    let mut index = open(build_archive(&[("mods/fabric-loader.jar", b"opaque")]));
    let parsed = normalize(&mut index, &options(GenericContentRoot::ArchiveRoot))
        .expect("names and payloads do not infer runtime");
    assert!(parsed.runtime().primary_loader().is_none());
}
