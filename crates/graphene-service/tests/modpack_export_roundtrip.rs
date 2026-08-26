//! Service-level composite round-trip coverage for the modpack export pipeline.
//!
//! Builds a fully materialized vanilla instance (descriptor, install receipt, schema-3
//! lockfile, instance files, cache-only artifact, seed file), plans an export against it,
//! executes the export into a create-only archive, and re-imports the produced archive
//! through the Graphene pack v1 normalizer to prove lossless round-tripping.

use graphene_core::{
    ArtifactIntegrity, ArtifactKind, ArtifactSource, CancellationToken, ErrorCode, InstanceId,
    Sha1Digest, Sha256Digest, Sha512Digest,
};
use graphene_instance::{
    InstallReceipt, InstalledArtifact, InstalledComponent, InstalledComponentKind,
    InstalledJavaRequirement, InstanceDescriptor, InstanceLockfile, LOCKFILE_SCHEMA_VERSION,
    LockedArtifact, LockedContentEntry, LockedMaterializationScope, LockedPackOrigin,
    ManagedRelativePath, NewInstanceSpec,
};
use graphene_modpack::{
    OptionalSelectionPolicy,
    archive::PackArchiveIndex,
    format::{detect_pack_format, graphene},
    model::{PackFormat, ProviderFileRef},
};
use graphene_providers::{CurseForgeProviderConfig, ModrinthProviderConfig};
use graphene_service::{
    ExportEmbeddingPolicy, Graphene, InstanceRepository, ModpackExportRequest,
    ModpackImportRequest, PackSource,
};
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};
use std::collections::BTreeSet;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const ALPHA_BYTES: &[u8] = b"roundtrip-alpha-payload";
const BETA_BYTES: &[u8] = b"roundtrip-beta-cache-only";
const GAMMA_BYTES: &[u8] = b"roundtrip-gamma-instance-copy";
const SEED_BYTES: &[u8] = b"render-distance:12\n";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sha256_hex(payload: &[u8]) -> String {
    let digest: [u8; 32] = Sha256::digest(payload).into();
    hex(&digest)
}

fn full_integrity(payload: &[u8]) -> ArtifactIntegrity {
    let sha1_bytes: [u8; 20] = Sha1::digest(payload).into();
    let sha256_bytes: [u8; 32] = Sha256::digest(payload).into();
    let sha512_bytes: [u8; 64] = Sha512::digest(payload).into();
    ArtifactIntegrity::default()
        .with_sha1(Sha1Digest::from_bytes(sha1_bytes))
        .with_sha256(Sha256Digest::from_bytes(sha256_bytes))
        .with_sha512(Sha512Digest::from_bytes(sha512_bytes))
}

/// Entry ids follow the planning scheme: `pk-` plus the first 16 hex chars of the SHA-256
/// of the destination string.
fn entry_id_for(destination: &str) -> String {
    format!("pk-{}", &sha256_hex(destination.as_bytes())[..16])
}

async fn build_engine(temp: &TempDir) -> Graphene {
    let mut builder = Graphene::builder(temp.path().to_path_buf());
    builder = builder.modrinth_provider(
        ModrinthProviderConfig::fixture("http://127.0.0.1:9").expect("fixture config"),
    );
    builder = builder.curseforge_provider(
        CurseForgeProviderConfig::fixture(None, "http://127.0.0.1:9").expect("fixture config"),
    );
    builder.build().await.expect("engine builds")
}

struct Fixture {
    _temp: TempDir,
    engine: Graphene,
    instance_id: InstanceId,
}

impl Fixture {
    fn root(&self) -> &Path {
        self._temp.path()
    }

    fn repo(&self) -> InstanceRepository {
        InstanceRepository::new(self.root())
    }
}

fn write_lockfile(fixture: &Fixture) {
    let lock_path = fixture.repo().paths().lockfile_path(fixture.instance_id);

    let managed = [
        ("mods/alpha.jar", ALPHA_BYTES, None),
        (
            "mods/beta.jar",
            BETA_BYTES,
            Some("https://cdn.example.com/beta.jar"),
        ),
        (
            "mods/gamma.jar",
            GAMMA_BYTES,
            Some("https://cdn.example.com/gamma.jar"),
        ),
    ];

    let mut artifacts = Vec::new();
    let mut content = Vec::new();
    for (destination, payload, source_url) in managed {
        let logical_key = format!("content:{destination}");
        let sources = source_url
            .map(|url| vec![ArtifactSource::new(url.to_owned())])
            .unwrap_or_default();
        artifacts.push(LockedArtifact {
            logical_key: logical_key.clone(),
            kind: ArtifactKind::Generic,
            sources,
            destination: ManagedRelativePath::new(format!(".minecraft/{destination}")).unwrap(),
            scope: LockedMaterializationScope::InstanceMutable,
            integrity: full_integrity(payload),
            expected_size: Some(payload.len() as u64),
        });
        let gamma_provenance = destination == "mods/gamma.jar";
        content.push(LockedContentEntry {
            entry_id: entry_id_for(destination),
            kind: "mod".to_owned(),
            provider: gamma_provenance.then(|| "curseforge".to_owned()),
            project_id: gamma_provenance.then(|| "42".to_owned()),
            version_id: gamma_provenance.then(|| "404242".to_owned()),
            file_id: gamma_provenance.then(|| "404242".to_owned()),
            artifact_logical_key: logical_key,
            destination: ManagedRelativePath::new(format!(".minecraft/{destination}")).unwrap(),
            enabled: true,
            dependencies: Vec::new(),
        });
    }

    let origin_sha256_bytes: [u8; 32] = Sha256::digest(b"origin-archive").into();
    let lockfile = InstanceLockfile {
        schema_version: LOCKFILE_SCHEMA_VERSION,
        instance_id: fixture.instance_id,
        minecraft_version: "1.21.1".to_owned(),
        components: vec![InstalledComponent {
            uid: "net.minecraft".to_owned(),
            version: "1.21.1".to_owned(),
            kind: InstalledComponentKind::Minecraft,
            provider: "mojang".to_owned(),
            provenance: None,
        }],
        artifacts,
        native_extractions: Vec::new(),
        generated_outputs: Vec::new(),
        content,
        pack_origin: Some(
            LockedPackOrigin::new(
                "GRAPHENE",
                Some("Round Trip".to_owned()),
                Some("1.0.0".to_owned()),
                Sha256Digest::from_bytes(origin_sha256_bytes),
                None,
                None,
            )
            .expect("valid pack origin"),
        ),
    };
    lockfile.validate().expect("fixture lockfile validates");
    graphene_storage::write_json_atomic(&lock_path, &lockfile).expect("lockfile written");
}

async fn build_fixture() -> Fixture {
    let temp = TempDir::new().expect("tempdir");
    let engine = build_engine(&temp).await;
    let instance_id = InstanceId::new();

    let repo = InstanceRepository::new(temp.path());
    let spec = NewInstanceSpec::with_id(instance_id, "Round Trip Instance").expect("spec");
    repo.write_descriptor(instance_id, &InstanceDescriptor::create(&spec))
        .expect("descriptor written");

    let receipt = InstallReceipt {
        schema_version: 2,
        install_format_version: 1,
        instance_id,
        requested_version: "1.21.1".to_owned(),
        resolved_version: "1.21.1".to_owned(),
        components: vec![InstalledComponent {
            uid: "net.minecraft".to_owned(),
            version: "1.21.1".to_owned(),
            kind: InstalledComponentKind::Minecraft,
            provider: "mojang".to_owned(),
            provenance: None,
        }],
        version_type: "release".to_owned(),
        main_class: "net.minecraft.client.main.Main".to_owned(),
        java_requirement: InstalledJavaRequirement {
            major_version: 21,
            component_hint: None,
        },
        client: InstalledArtifact {
            path: ManagedRelativePath::new(".minecraft/versions/1.21.1/1.21.1.jar").unwrap(),
            integrity: ArtifactIntegrity::default(),
            expected_size: Some(100),
        },
        libraries: Vec::new(),
        asset_index_id: "21".to_owned(),
        asset_index: InstalledArtifact {
            path: ManagedRelativePath::new("shared/assets/indexes/21.json").unwrap(),
            integrity: ArtifactIntegrity::default(),
            expected_size: Some(100),
        },
        assets_root: ManagedRelativePath::new("shared/assets").unwrap(),
        natives_directory: ManagedRelativePath::new(".graphene/natives/1.21.1").unwrap(),
        logging_configuration: None,
        logging_argument: None,
        jvm_arguments: Vec::new(),
        game_arguments: Vec::new(),
    };
    graphene_storage::write_json_atomic(&repo.paths().install_receipt_path(instance_id), &receipt)
        .expect("receipt written");

    let fixture = Fixture {
        _temp: temp,
        engine,
        instance_id,
    };
    write_lockfile(&fixture);

    // Instance copies for alpha and gamma; beta stays cache-only.
    let minecraft_dir = fixture.repo().paths().minecraft_dir(instance_id);
    fs::create_dir_all(minecraft_dir.join("mods")).expect("mods dir");
    fs::write(minecraft_dir.join("mods/alpha.jar"), ALPHA_BYTES).expect("alpha written");
    fs::write(minecraft_dir.join("mods/gamma.jar"), GAMMA_BYTES).expect("gamma written");

    // Commit beta into the managed cache at its integrity-addressed location.
    let data_root = graphene_storage::DataRoot::initialize(fixture.root()).expect("data root");
    let address = data_root
        .cache_address(&full_integrity(BETA_BYTES))
        .expect("cache address");
    let beta_path = address.path().to_path_buf();
    fs::create_dir_all(beta_path.parent().unwrap()).expect("cache parent");
    fs::write(&beta_path, BETA_BYTES).expect("beta cached");

    // Seed payload.
    fs::create_dir_all(minecraft_dir.join("config")).expect("config dir");
    fs::write(minecraft_dir.join("config/options.txt"), SEED_BYTES).expect("seed written");

    fixture
}

fn export_request(instance_id: InstanceId, output: PathBuf) -> ModpackExportRequest {
    let mut embed = BTreeSet::new();
    embed.insert("mods/alpha.jar".to_owned());
    ModpackExportRequest::new(
        instance_id,
        "Round Trip",
        Some("1.0.0".to_owned()),
        Some("integration fixture".to_owned()),
        output,
        ExportEmbeddingPolicy::EmbedExplicit(embed),
    )
    .expect("request builds")
}

async fn plan_and_execute(
    fixture: &Fixture,
    output: PathBuf,
    seed: bool,
) -> graphene_service::ModpackExportResult {
    let request = {
        let request = export_request(fixture.instance_id, output);
        if seed {
            request
                .with_seed("config/options.txt")
                .expect("seed accepted")
        } else {
            request
        }
    };

    let plan = fixture
        .engine
        .modpacks()
        .plan_export(request)
        .await_result()
        .await
        .expect("export plan");

    if seed {
        assert_eq!(plan.managed_count(), 3, "all enabled entries planned");
        assert_eq!(plan.embedded_count(), 1, "only alpha embedded");
        assert_eq!(plan.seed_count(), 1);
        assert!(plan.diagnostics().is_empty(), "no source-less entries");
    }

    fixture
        .engine
        .modpacks()
        .execute_export(plan)
        .await_result()
        .await
        .expect("export executes")
}
#[tokio::test]
async fn export_round_trips_lockfile_state_into_a_valid_graphene_pack() {
    let fixture = build_fixture().await;
    let temp_out = TempDir::new().expect("out tempdir");
    let output = temp_out.path().join("pack.zip");

    let result = plan_and_execute(&fixture, output.clone(), true).await;
    assert_eq!(result.referenced_managed_files, 3);
    assert_eq!(result.embedded_objects, 1);
    assert_eq!(result.seed_files, 1);
    assert_eq!(
        result.archive_sha256,
        sha256_hex(&fs::read(&output).expect("archive readable"))
    );

    // Round-trip: the exported archive must import cleanly as a Graphene pack.
    let archive = fs::read(&output).expect("archive bytes");
    let mut index =
        PackArchiveIndex::open(Cursor::new(archive), &CancellationToken::new()).expect("opens");
    let detection = detect_pack_format(&mut index, false).expect("detected");
    assert_eq!(detection.format, PackFormat::Graphene);
    let normalized =
        graphene::normalize(&mut index, &detection.root_prefix).expect("exported pack re-imports");

    // Only the source-backed, provenance-less file remains a managed remote entry:
    // alpha is embedded payload and gamma resolves through provider provenance.
    let destinations: Vec<String> = normalized
        .managed_files()
        .iter()
        .map(|file| file.destination().as_str().to_owned())
        .collect();
    assert_eq!(destinations, vec!["mods/beta.jar".to_owned()]);
    let beta = &normalized.managed_files()[0];
    assert!(!beta.sources().is_empty(), "beta keeps its source URL");

    let embedded: Vec<String> = normalized
        .embedded_files()
        .iter()
        .map(|file| file.destination().as_str().to_owned())
        .collect();
    assert_eq!(embedded, vec!["mods/alpha.jar".to_owned()]);

    // Provenance round-trips into a pending provider reference.
    let pending = normalized.pending_provider_files();
    assert_eq!(pending.len(), 1, "gamma provenance survives the round trip");
    match pending[0].provider_ref() {
        ProviderFileRef::CurseForge {
            project_id,
            file_id,
        } => {
            assert_eq!(project_id, "42");
            assert_eq!(file_id, "404242");
        }
        _ => panic!("expected a CurseForge provider reference"),
    }

    let seed_destinations: Vec<String> = normalized
        .seed_entries()
        .iter()
        .map(|entry| entry.destination().as_str().to_owned())
        .collect();
    assert_eq!(seed_destinations, vec!["config/options.txt".to_owned()]);
}

#[tokio::test]
async fn export_fails_stale_when_instance_bytes_change_after_planning() {
    let fixture = build_fixture().await;
    let temp_out = TempDir::new().expect("out tempdir");
    let output = temp_out.path().join("stale.zip");

    let request = export_request(fixture.instance_id, output);
    let plan = fixture
        .engine
        .modpacks()
        .plan_export(request)
        .await_result()
        .await
        .expect("plan");

    let minecraft_dir = fixture.repo().paths().minecraft_dir(fixture.instance_id);
    fs::write(minecraft_dir.join("mods/alpha.jar"), b"TAMPERED").expect("tamper");

    let error = fixture
        .engine
        .modpacks()
        .execute_export(plan)
        .await_result()
        .await
        .expect_err("stale instance must fail export");
    assert_eq!(error.code, ErrorCode::PackExportStale);
}

#[tokio::test]
async fn export_publication_is_create_only() {
    let fixture = build_fixture().await;
    let temp_out = TempDir::new().expect("out tempdir");
    let output = temp_out.path().join("create-only.zip");

    let request = export_request(fixture.instance_id, output.clone());
    let plan = fixture
        .engine
        .modpacks()
        .plan_export(request)
        .await_result()
        .await
        .expect("plan");
    let replay_plan = plan.clone();

    let result = fixture
        .engine
        .modpacks()
        .execute_export(plan)
        .await_result()
        .await
        .expect("first publication succeeds");
    assert!(output.is_file());
    assert_eq!(result.referenced_managed_files, 3);

    let error = fixture
        .engine
        .modpacks()
        .execute_export(replay_plan)
        .await_result()
        .await
        .expect_err("second publication must refuse to overwrite");
    assert_eq!(error.code, ErrorCode::PackExportInvalid);
}

#[tokio::test]
async fn plan_import_rejects_existing_target_structurally() {
    let fixture = build_fixture().await;
    let temp_out = TempDir::new().expect("out tempdir");
    let output = temp_out.path().join("import-src.zip");

    plan_and_execute(&fixture, output.clone(), false).await;

    let inspection = fixture
        .engine
        .modpacks()
        .inspect(PackSource::LocalFile(output))
        .await_result()
        .await
        .expect("inspection succeeds offline");
    assert_eq!(inspection.format(), PackFormat::Graphene);

    // The target already exists, so planning must fail structurally before any
    // provider or network interaction.
    let target =
        NewInstanceSpec::with_id(fixture.instance_id, "Duplicate Target").expect("target spec");
    let import_request = ModpackImportRequest {
        snapshot: inspection.snapshot().clone(),
        target,
        optional_policy: OptionalSelectionPolicy::RequiredOnly,
    };

    let error = fixture
        .engine
        .modpacks()
        .plan_import(import_request)
        .await_result()
        .await
        .expect_err("existing target must fail");
    assert_eq!(error.code, ErrorCode::InstallTargetExists);
}

#[tokio::test]
async fn inspect_promotes_multimc_embedded_mods_instead_of_rejecting_them() {
    use graphene_core::archive::DeterministicZipWriter;
    use graphene_modpack::model::PackFormat as Format;

    let fixture = build_fixture().await;
    let manifest =
        r#"{"formatVersion":1,"components":[{"uid":"net.minecraft","version":"1.21.1"}]}"#;
    let entries = vec![
        ("mmc-pack.json".to_owned(), manifest.as_bytes().to_vec()),
        (".minecraft/".to_owned(), Vec::new()),
        (
            ".minecraft/mods/promoted.jar".to_owned(),
            b"embedded-mod-bytes".to_vec(),
        ),
        (
            ".minecraft/config/options.txt".to_owned(),
            b"seed-me\n".to_vec(),
        ),
    ];
    let mut writer = DeterministicZipWriter::new(Cursor::new(Vec::new()));
    for (name, payload) in &entries {
        writer.add_entry(name, payload).expect("fixture entry");
    }
    writer.finish().expect("finish");
    let archive_path = fixture.root().join("multimc-fixture.zip");
    fs::write(
        &archive_path,
        writer.into_inner().expect("finalized").into_inner(),
    )
    .expect("fixture written");

    let inspection = fixture
        .engine
        .modpacks()
        .inspect(PackSource::LocalFile(archive_path))
        .await_result()
        .await
        .expect("MultiMC inspection succeeds");
    assert_eq!(inspection.format(), Format::PrismMultiMc);
    assert_eq!(inspection.embedded_mod_count(), 1);
    assert!(inspection.seed_entry_count() >= 1);
}
