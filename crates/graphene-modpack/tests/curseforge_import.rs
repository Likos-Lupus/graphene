use graphene_core::archive::DeterministicZipWriter;
use graphene_core::{CancellationToken, ErrorCode, Sha256Digest};
use graphene_minecraft::LoaderKind;
use graphene_modpack::archive::PackArchiveIndex;
use graphene_modpack::format::{curseforge, detect_pack_format};
use graphene_modpack::model::ContentHint;
use graphene_modpack::{
    FileSelection, OptionalSelectionPolicy, PackDiagnosticCode, PackFormat, ProviderFileRef,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::Cursor;

fn manifest(minecraft: &str, loaders: &str, files: &str, extra: &str) -> String {
    format!(
        r#"{{"minecraft":{{"version":"{minecraft}","modLoaders":[{loaders}]}},"manifestType":"minecraftModpack","manifestVersion":1,"name":"Fixture Pack","version":"2.0","author":"Graphene Tests","files":[{files}]{extra}}}"#
    )
}

fn file(project: u64, file: u64, required: bool) -> String {
    format!(r#"{{"projectID":{project},"fileID":{file},"required":{required}}}"#)
}

fn loader(id: &str, primary: bool) -> String {
    format!(r#"{{"id":"{id}","primary":{primary}}}"#)
}

fn build_archive(entries: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    let mut writer = DeterministicZipWriter::new(Cursor::new(Vec::new()));
    for (name, payload) in entries {
        writer.add_entry(&name, &payload).expect("fixture entry");
    }
    writer.finish().expect("finish");
    writer.into_inner().expect("finalized").into_inner()
}

fn pack(manifest: String, mut entries: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    entries.insert(0, ("manifest.json".to_owned(), manifest.into_bytes()));
    build_archive(entries)
}

fn normalize(
    bytes: Vec<u8>,
    policy: &OptionalSelectionPolicy,
) -> Result<graphene_modpack::NormalizedModpack, graphene_modpack::PackError> {
    let mut index = PackArchiveIndex::open(Cursor::new(bytes), &CancellationToken::new())
        .expect("archive opens");
    let detection = detect_pack_format(&mut index, false).expect("detection");
    assert_eq!(detection.format, PackFormat::CurseForge);
    curseforge::normalize(&mut index, &detection.root_prefix, policy)
}

fn required_only() -> OptionalSelectionPolicy {
    OptionalSelectionPolicy::RequiredOnly
}

fn normalize_direct(
    manifest: String,
) -> Result<graphene_modpack::NormalizedModpack, graphene_modpack::PackError> {
    let mut index = PackArchiveIndex::open(
        Cursor::new(pack(manifest, Vec::new())),
        &CancellationToken::new(),
    )
    .expect("archive opens");
    curseforge::normalize(&mut index, "", &required_only())
}

#[test]
fn required_and_optional_provider_references_normalize_stably() {
    let files = format!("{},{}", file(22, 220, false), file(11, 110, true));
    let normalized = normalize(
        pack(manifest("1.21.1", "", &files, ""), Vec::new()),
        &OptionalSelectionPolicy::IncludeAllOptional,
    )
    .expect("normalization");

    assert_eq!(normalized.metadata().name(), "Fixture Pack");
    assert_eq!(normalized.metadata().version(), Some("2.0"));
    assert_eq!(normalized.metadata().authors(), ["Graphene Tests"]);
    assert_eq!(normalized.runtime().minecraft_version(), "1.21.1");
    assert!(normalized.runtime().primary_loader().is_none());
    assert_eq!(normalized.pending_provider_files().len(), 2);
    assert_eq!(
        normalized.pending_provider_files()[0].stable_id(),
        "curseforge:11:110"
    );
    assert!(matches!(
        normalized.pending_provider_files()[0].selection(),
        FileSelection::Required
    ));
    assert_eq!(
        normalized.pending_provider_files()[1].provider_ref(),
        &ProviderFileRef::CurseForge {
            project_id: "22".to_owned(),
            file_id: "220".to_owned(),
        }
    );
    assert!(matches!(
        normalized.pending_provider_files()[1].selection(),
        FileSelection::Optional { choice_id } if choice_id == "curseforge:22:220"
    ));
    assert_eq!(normalized.optional_choices().len(), 1);
    assert_eq!(normalized.optional_choices()[0].id(), "curseforge:22:220");
    assert!(normalized.optional_choices()[0].destination().is_none());
    assert!(normalized.optional_choices()[0].size().is_none());
    assert!(!normalized.optional_choices()[0].default_selected());
    assert!(normalized.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == PackDiagnosticCode::SourceHasNoAuthenticityProof
    }));
}

#[test]
fn optional_selection_policies_include_or_skip_provider_files() {
    let files = format!("{},{}", file(1, 10, true), file(2, 20, false));
    let bytes = pack(manifest("1.20.1", "", &files, ""), Vec::new());

    let required = normalize(bytes.clone(), &required_only()).expect("required only");
    assert_eq!(required.pending_provider_files().len(), 1);
    assert_eq!(required.optional_choices().len(), 1);
    assert!(
        required
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == PackDiagnosticCode::OptionalFileNotSelected })
    );

    let all = normalize(bytes.clone(), &OptionalSelectionPolicy::IncludeAllOptional)
        .expect("include all");
    assert_eq!(all.pending_provider_files().len(), 2);

    let unselected = normalize(
        bytes.clone(),
        &OptionalSelectionPolicy::Explicit(BTreeSet::new()),
    )
    .expect("explicit exclusion");
    assert_eq!(unselected.pending_provider_files().len(), 1);
    assert!(
        unselected
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == PackDiagnosticCode::OptionalFileNotSelected })
    );

    let mut selected = BTreeSet::new();
    selected.insert("curseforge:2:20".to_owned());
    let explicit =
        normalize(bytes, &OptionalSelectionPolicy::Explicit(selected)).expect("explicit inclusion");
    assert_eq!(explicit.pending_provider_files().len(), 2);
    assert!(matches!(
        explicit.pending_provider_files()[1].selection(),
        FileSelection::Optional { choice_id } if choice_id == "curseforge:2:20"
    ));
}

#[test]
fn supported_loader_forms_map_to_exact_runtime_requirements() {
    for (minecraft, id, kind, version) in [
        ("1.21.1", "fabric-0.16.9", LoaderKind::Fabric, "0.16.9"),
        (
            "1.21.1",
            "neoforge-21.1.37",
            LoaderKind::NeoForge,
            "21.1.37",
        ),
        ("1.20.1", "forge-47.3.0", LoaderKind::Forge, "47.3.0"),
        ("1.20.1", "forge-1.20.1-47.3.0", LoaderKind::Forge, "47.3.0"),
    ] {
        let loader = loader(id, true);
        let normalized = normalize(
            pack(manifest(minecraft, &loader, "", ""), Vec::new()),
            &required_only(),
        )
        .expect(id);
        let actual = normalized
            .runtime()
            .primary_loader()
            .expect("primary loader");
        assert_eq!(actual.kind(), kind);
        assert_eq!(actual.version(), version);
    }
}

#[test]
fn unsupported_loader_compositions_fail_closed() {
    for loaders in [
        loader("quilt-0.26.0", true),
        loader("unknown-1.0", true),
        loader("fabric-latest", true),
        loader("fabric-0.16.9", false),
        format!(
            "{},{}",
            loader("fabric-0.16.9", true),
            loader("forge-47.3.0", true)
        ),
        format!(
            "{},{}",
            loader("fabric-0.16.9", true),
            loader("forge-47.3.0", false)
        ),
    ] {
        let error = normalize(
            pack(manifest("1.20.1", &loaders, "", ""), Vec::new()),
            &required_only(),
        )
        .expect_err("unsupported composition");
        assert_eq!(error.code(), ErrorCode::PackRuntimeUnsupported);
    }
}

#[test]
fn duplicate_and_out_of_range_provider_ids_are_rejected() {
    let duplicate = format!("{},{}", file(7, 8, true), file(7, 8, false));
    let error = normalize(
        pack(manifest("1.20.1", "", &duplicate, ""), Vec::new()),
        &required_only(),
    )
    .expect_err("duplicate pair");
    assert_eq!(error.code(), ErrorCode::PackManifestInvalid);

    for files in [
        file(0, 1, true),
        file(1, 0, true),
        file(u64::from(u32::MAX) + 1, 1, true),
    ] {
        let error = normalize(
            pack(manifest("1.20.1", "", &files, ""), Vec::new()),
            &required_only(),
        )
        .expect_err("invalid id");
        assert_eq!(error.code(), ErrorCode::PackManifestInvalid);
    }
}

#[test]
fn manifest_bounds_identity_and_override_root_are_validated() {
    let wrong_type = manifest("1.20.1", "", "", "").replace(
        "\"manifestType\":\"minecraftModpack\"",
        "\"manifestType\":\"other\"",
    );
    let error = normalize_direct(wrong_type).expect_err("type");
    assert_eq!(error.code(), ErrorCode::PackManifestInvalid);

    let wrong_version =
        manifest("1.20.1", "", "", "").replace("\"manifestVersion\":1", "\"manifestVersion\":2");
    let error = normalize_direct(wrong_version).expect_err("version");
    assert_eq!(error.code(), ErrorCode::PackManifestInvalid);

    let long_name = "x".repeat(257);
    let error = normalize(
        pack(
            manifest("1.20.1", "", "", "").replace("Fixture Pack", &long_name),
            Vec::new(),
        ),
        &required_only(),
    )
    .expect_err("name bound");
    assert_eq!(error.code(), ErrorCode::PackManifestInvalid);

    let error = normalize(
        pack(
            manifest("1.20.1", "", "", r#","overrides":"Override""#),
            Vec::new(),
        ),
        &required_only(),
    )
    .expect_err("override root");
    assert_eq!(error.code(), ErrorCode::PackManifestInvalid);
}

#[test]
fn wrapper_overrides_are_stripped_hashed_and_sorted() {
    let manifest = manifest("1.21.1", "", "", r#","overrides":"overrides""#);
    let bytes = build_archive(vec![
        ("Wrapped/manifest.json".to_owned(), manifest.into_bytes()),
        (
            "Wrapped/overrides/options.txt".to_owned(),
            b"music:0.5".to_vec(),
        ),
        (
            "Wrapped/overrides/config/example.toml".to_owned(),
            b"enabled=true".to_vec(),
        ),
    ]);
    let normalized = normalize(bytes, &required_only()).expect("wrapped normalization");

    let destinations: Vec<_> = normalized
        .seed_entries()
        .iter()
        .map(|entry| entry.destination().as_str())
        .collect();
    assert_eq!(destinations, ["config/example.toml", "options.txt"]);
    let expected = Sha256Digest::from_bytes(Sha256::digest(b"enabled=true").into());
    assert_eq!(normalized.seed_entries()[0].sha256(), &expected);
    assert_eq!(normalized.seed_entries()[0].size(), 12);
    assert_eq!(
        normalized.seed_entries()[0].archive_entry(),
        "Wrapped/overrides/config/example.toml"
    );
}

#[test]
fn unsafe_special_and_case_colliding_override_entries_are_rejected() {
    let unsafe_bytes = pack(
        manifest("1.20.1", "", "", ""),
        vec![("overrides/../escape.txt".to_owned(), b"escape".to_vec())],
    );
    let error = normalize(unsafe_bytes, &required_only()).expect_err("unsafe path");
    assert_eq!(error.code(), ErrorCode::PackPathInvalid);

    let collision = pack(
        manifest("1.20.1", "", "", ""),
        vec![
            ("overrides/config/A.toml".to_owned(), b"a".to_vec()),
            ("overrides/CONFIG/a.toml".to_owned(), b"b".to_vec()),
        ],
    );
    let error = normalize(collision, &required_only()).expect_err("case collision");
    assert_eq!(error.code(), ErrorCode::PackPathInvalid);

    let ordinary = pack(
        manifest("1.20.1", "", "", ""),
        vec![("overrides/mods/link.jar".to_owned(), b"target".to_vec())],
    );
    let symlink = mark_entry_mode(ordinary, "overrides/mods/link.jar", 0o120777);
    let error = normalize(symlink, &required_only()).expect_err("symlink");
    assert_eq!(error.code(), ErrorCode::PackArchiveInvalid);

    let ordinary = pack(
        manifest("1.20.1", "", "", ""),
        vec![("overrides/config/pipe".to_owned(), Vec::new())],
    );
    let special = mark_entry_mode(ordinary, "overrides/config/pipe", 0o010644);
    let error = normalize(special, &required_only()).expect_err("special entry");
    assert_eq!(error.code(), ErrorCode::PackArchiveInvalid);
}

#[test]
fn embedded_override_mods_are_promoted_including_disabled_jars() {
    let normalized = normalize(
        pack(
            manifest("1.20.1", "", "", ""),
            vec![
                ("overrides/mods/enabled.jar".to_owned(), b"enabled".to_vec()),
                (
                    "overrides/mods/disabled.jar.disabled".to_owned(),
                    b"disabled".to_vec(),
                ),
                ("overrides/mods/readme.txt".to_owned(), b"seed".to_vec()),
            ],
        ),
        &required_only(),
    )
    .expect("embedded mods");

    let embedded: Vec<_> = normalized
        .embedded_files()
        .iter()
        .map(|entry| entry.destination().as_str())
        .collect();
    assert_eq!(embedded, ["mods/disabled.jar.disabled", "mods/enabled.jar"]);
    assert!(
        normalized
            .embedded_files()
            .iter()
            .all(|entry| entry.content_hint() == Some(ContentHint::Mod))
    );
    assert_eq!(
        normalized.seed_entries()[0].destination().as_str(),
        "mods/readme.txt"
    );
    assert!(normalized.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == PackDiagnosticCode::EmbeddedModHasNoProviderIdentity
    }));
}

fn mark_entry_mode(mut archive: Vec<u8>, entry_name: &str, unix_mode: u16) -> Vec<u8> {
    let name = entry_name.as_bytes();
    let central = [0x50, 0x4b, 0x01, 0x02];
    let offset = archive
        .windows(central.len())
        .enumerate()
        .filter(|(_, window)| *window == central)
        .map(|(offset, _)| offset)
        .find(|offset| {
            let name_len =
                u16::from_le_bytes([archive[offset + 28], archive[offset + 29]]) as usize;
            &archive[offset + 46..offset + 46 + name_len] == name
        })
        .expect("entry central directory record");
    archive[offset + 4..offset + 6].copy_from_slice(&((3_u16 << 8) | 20).to_le_bytes());
    archive[offset + 38..offset + 42].copy_from_slice(&(u32::from(unix_mode) << 16).to_le_bytes());
    archive
}
