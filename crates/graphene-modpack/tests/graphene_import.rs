use graphene_core::CancellationToken;
use graphene_modpack::archive::PackArchiveIndex;
use graphene_modpack::format::{detect_pack_format, graphene};
use graphene_modpack::model::{FileSelection, PackFormat, ProviderFileRef};
use std::io::Cursor;

const MANIFEST_NAME: &str = "graphene.pack.json";

fn sha256_hex(payload: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(payload);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn build_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = graphene_core::archive::DeterministicZipWriter::new(Cursor::new(Vec::new()));
    for (name, payload) in entries {
        writer.add_entry(name, payload).expect("fixture entry");
    }
    writer.finish().expect("finish");
    writer.into_inner().expect("finalized").into_inner()
}

fn object_entry_name(hex: &str) -> String {
    format!("objects/sha256/{}/{}", &hex[..2], hex)
}

struct Fixture {
    seed_payload: Vec<u8>,
    object_payload: Vec<u8>,
}

impl Fixture {
    fn new() -> Self {
        Self {
            seed_payload: b"music:0.4\n".to_vec(),
            object_payload: b"PK\x03\x04 embedded jar bytes".to_vec(),
        }
    }

    fn seed_sha(&self) -> String {
        sha256_hex(&self.seed_payload)
    }

    fn object_sha(&self) -> String {
        sha256_hex(&self.object_payload)
    }

    fn manifest(&self, managed_files: &[String], seed_files: &[String]) -> String {
        format!(
            r#"{{"schema_version":1,"pack":{{"name":"Graphene Fixture","version":"1.0","summary":"fixture"}},"runtime":{{"minecraft_version":"1.21.1","primary_loader":{{"kind":"fabric","exact_version":"0.16.9"}}}},"managed_files":[{}],"seed_files":[{}]}}"#,
            managed_files.join(","),
            seed_files.join(",")
        )
    }

    fn remote_managed(&self) -> String {
        format!(
            r#"{{"destination":"mods/remote.jar","sha256":"{}","size":1024,"sources":["https://mirror.example.com/files/remote.jar"]}}"#,
            "aa".repeat(32)
        )
    }

    fn embedded_managed(&self) -> String {
        let hex = self.object_sha();
        format!(
            r#"{{"destination":"mods/embedded.jar","sha256":"{hex}","size":{size},"embedded_object":"{hex}"}}"#,
            size = self.object_payload.len(),
        )
    }

    fn provenance_managed(&self) -> String {
        format!(
            r#"{{"destination":"mods/provided.jar","sha256":"{}","size":2048,"content_provenance":{{"provider":"curseforge","project_id":"238222","version_id":"5112345"}}}}"#,
            "bb".repeat(32)
        )
    }

    fn seed_file(&self) -> String {
        let hex = self.seed_sha();
        format!(
            r#"{{"destination":"options.txt","sha256":"{hex}","size":{size},"archive_entry":"seed/options.txt"}}"#,
            size = self.seed_payload.len(),
        )
    }

    fn pack(&self, managed_files: &[String], seed_files: &[String]) -> Vec<u8> {
        let manifest = self.manifest(managed_files, seed_files);
        let object = object_entry_name(&self.object_sha());
        build_archive(&[
            (MANIFEST_NAME, manifest.as_bytes()),
            (object.as_str(), &self.object_payload),
            ("seed/options.txt", &self.seed_payload),
        ])
    }
}

fn normalize(
    bytes: Vec<u8>,
) -> Result<graphene_modpack::NormalizedModpack, graphene_modpack::PackError> {
    let mut index =
        PackArchiveIndex::open(Cursor::new(bytes), &CancellationToken::new()).expect("opens");
    let detection = detect_pack_format(&mut index, false)?;
    assert_eq!(detection.format, PackFormat::Graphene);
    assert_eq!(detection.root_prefix, "");
    graphene::normalize(&mut index, &detection.root_prefix)
}

#[test]
fn minimal_valid_pack_normalizes_all_strategies() {
    let fixture = Fixture::new();
    let bytes = fixture.pack(
        &[
            fixture.remote_managed(),
            fixture.embedded_managed(),
            fixture.provenance_managed(),
        ],
        &[fixture.seed_file()],
    );

    let normalized = normalize(bytes).expect("normalization");
    assert_eq!(normalized.format(), PackFormat::Graphene);
    assert_eq!(normalized.metadata().name(), "Graphene Fixture");
    assert_eq!(normalized.runtime().minecraft_version(), "1.21.1");
    let loader = normalized.runtime().primary_loader().expect("loader");
    assert_eq!(loader.version(), "0.16.9");

    assert_eq!(normalized.managed_files().len(), 1);
    assert_eq!(
        normalized.managed_files()[0].destination().as_str(),
        "mods/remote.jar"
    );
    assert!(matches!(
        normalized.managed_files()[0].selection(),
        FileSelection::Required
    ));

    assert_eq!(normalized.embedded_files().len(), 1);
    let embedded = &normalized.embedded_files()[0];
    assert_eq!(embedded.destination().as_str(), "mods/embedded.jar");
    assert_eq!(
        embedded.archive_entry(),
        object_entry_name(&fixture.object_sha())
    );

    assert_eq!(normalized.pending_provider_files().len(), 1);
    match normalized.pending_provider_files()[0].provider_ref() {
        ProviderFileRef::CurseForge {
            project_id,
            file_id,
        } => {
            assert_eq!(project_id, "238222");
            assert_eq!(file_id, "5112345");
        }
        other => panic!("unexpected provider ref: {other:?}"),
    }

    assert_eq!(normalized.seed_entries().len(), 1);
    assert_eq!(
        normalized.seed_entries()[0].destination().as_str(),
        "options.txt"
    );
}

#[test]
fn multiple_acquisition_strategies_fail_closed() {
    let fixture = Fixture::new();
    let hex = "cc".repeat(32);
    let managed = format!(
        r#"{{"destination":"mods/dual.jar","sha256":"{hex}","size":10,"sources":["https://mirror.example.com/a"],"embedded_object":"{hex}"}}"#
    );
    let error = normalize(fixture.pack(&[managed], &[])).expect_err("dual strategy must fail");
    assert!(error.to_string().contains("exactly one"));
}

#[test]
fn embedded_identity_mismatch_fails() {
    let fixture = Fixture::new();
    let file_hex = "dd".repeat(32);
    let object_hex = fixture.object_sha();
    let managed = format!(
        r#"{{"destination":"mods/mismatch.jar","sha256":"{file_hex}","size":10,"embedded_object":"{object_hex}"}}"#
    );
    let error = normalize(fixture.pack(&[managed], &[])).expect_err("mismatch must fail");
    assert!(error.to_string().contains("identity"));
}

#[test]
fn embedded_object_bytes_are_verified_against_the_declaration() {
    let fixture = Fixture::new();
    let corrupted = {
        let mut payload = fixture.object_payload.clone();
        payload[4] ^= 0xFF;
        payload
    };
    let hex = fixture.object_sha();
    let managed = fixture.embedded_managed();
    let object = object_entry_name(&hex);
    build_archive(&[
        (MANIFEST_NAME, fixture.manifest(&[managed], &[]).as_bytes()),
        (object.as_str(), &corrupted),
    ]);

    // Rebuild with the manifest declaring the ORIGINAL digest; the stored bytes are corrupt.
    let bytes = {
        let manifest = fixture.manifest(
            &[format!(
                r#"{{"destination":"mods/embedded.jar","sha256":"{hex}","size":{size},"embedded_object":"{hex}"}}"#,
                size = fixture.object_payload.len(),
            )],
            &[],
        );
        build_archive(&[
            (MANIFEST_NAME, manifest.as_bytes()),
            (object.as_str(), &corrupted),
            ("seed/options.txt", &fixture.seed_payload),
        ])
    };

    let error = normalize(bytes).expect_err("corrupt object must fail");
    assert!(error.to_string().contains("SHA-256") || error.to_string().contains("hash"));
}

#[test]
fn embedded_object_must_live_at_its_hash_derived_path() {
    let fixture = Fixture::new();
    let hex = fixture.object_sha();
    let managed = format!(
        r#"{{"destination":"mods/embedded.jar","sha256":"{hex}","size":{size},"embedded_object":"{hex}"}}"#,
        size = fixture.object_payload.len(),
    );
    let bytes = build_archive(&[
        (
            MANIFEST_NAME,
            fixture
                .manifest(&[managed], &[fixture.seed_file()])
                .as_bytes(),
        ),
        ("objects/sha256/zz/wrong-name", &fixture.object_payload),
        ("seed/options.txt", &fixture.seed_payload),
    ]);
    let error = normalize(bytes).expect_err("misplaced object must fail");
    assert!(error.to_string().contains("absent") || error.to_string().contains("unlisted"));
}

#[test]
fn seed_entry_must_use_derived_archive_path() {
    let fixture = Fixture::new();
    let hex = fixture.seed_sha();
    let seed = format!(
        r#"{{"destination":"options.txt","sha256":"{hex}","size":{size},"archive_entry":"elsewhere/options.txt"}}"#,
        size = fixture.seed_payload.len(),
    );
    let error = normalize(fixture.pack(&[], &[seed])).expect_err("aliased entry must fail");
    assert!(error.to_string().contains("derived path"));
}

#[test]
fn unlisted_seed_payload_fails_import() {
    let fixture = Fixture::new();
    let bytes = build_archive(&[
        (
            MANIFEST_NAME,
            fixture.manifest(&[], &[fixture.seed_file()]).as_bytes(),
        ),
        ("seed/options.txt", &fixture.seed_payload),
        ("seed/sneaky.cfg", b"hidden"),
    ]);
    let error = normalize(bytes).expect_err("unlisted seed must fail");
    assert!(error.to_string().contains("unlisted seed"));
}

#[test]
fn foreign_and_unreferenced_payloads_fail() {
    let fixture = Fixture::new();
    let bytes = build_archive(&[
        (
            MANIFEST_NAME,
            fixture.manifest(&[], &[fixture.seed_file()]).as_bytes(),
        ),
        ("seed/options.txt", &fixture.seed_payload),
        ("random/root-file.txt", b"foreign"),
    ]);
    let error = normalize(bytes).expect_err("foreign payload must fail");
    assert!(error.to_string().contains("outside reserved areas"));

    // An unreferenced embedded object is equally rejected.
    let unreferenced = build_archive(&[
        (
            MANIFEST_NAME,
            fixture.manifest(&[], &[fixture.seed_file()]).as_bytes(),
        ),
        ("seed/options.txt", &fixture.seed_payload),
        (
            object_entry_name(&fixture.object_sha()).as_str(),
            &fixture.object_payload,
        ),
    ]);
    let error = normalize(unreferenced).expect_err("unreferenced object must fail");
    assert!(error.to_string().contains("unlisted embedded object"));
}

#[test]
fn unknown_schema_version_fails_cleanly() {
    let fixture = Fixture::new();
    let manifest = fixture
        .manifest(&[], &[fixture.seed_file()])
        .replace("\"schema_version\":1", "\"schema_version\":2");
    let bytes = build_archive(&[
        (MANIFEST_NAME, manifest.as_bytes()),
        ("seed/options.txt", &fixture.seed_payload),
    ]);

    // Detection no longer claims the Graphene format for an unrecognized schema.
    let mut index = PackArchiveIndex::open(Cursor::new(bytes.clone()), &CancellationToken::new())
        .expect("opens");
    let detection = detect_pack_format(&mut index, false);
    assert!(detection.is_err());

    // Direct normalization still fails with a clean schema error rather than best-effort parsing.
    let mut index =
        PackArchiveIndex::open(Cursor::new(bytes), &CancellationToken::new()).expect("opens");
    let error = graphene::normalize(&mut index, "").expect_err("schema mismatch must fail");
    assert!(error.to_string().contains("schema version"));
}

#[test]
fn unsupported_loader_and_unsafe_source_fail_closed() {
    let fixture = Fixture::new();
    let quilt = fixture
        .manifest(&[], &[])
        .replace("\"fabric\"", "\"quilt\"");
    let bytes = build_archive(&[(MANIFEST_NAME, quilt.as_bytes())]);
    let error = normalize(bytes).expect_err("quilt loader must fail");
    assert!(error.to_string().contains("unsupported primary loader"));

    let unsafe_source = format!(
        r#"{{"destination":"mods/insecure.jar","sha256":"{}","size":8,"sources":["http://insecure.example.com/a"]}}"#,
        "ee".repeat(32)
    );
    let bytes = fixture.pack(&[unsafe_source], &[]);
    let error = normalize(bytes).expect_err("plain-http source must fail");
    assert!(error.to_string().contains("HTTPS"));
}
