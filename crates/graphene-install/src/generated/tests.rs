use super::*;
use graphene_minecraft::{
    ComponentKind, ComponentProvenance, ComponentUid, ComponentVersion, ManagedPath,
    ResolvedComponent,
};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("graphene-generated-output-{name}-{unique}"));
    fs::create_dir_all(&path).expect("temp root");
    path
}

fn component() -> ResolvedComponent {
    ResolvedComponent {
        uid: ComponentUid::new("net.minecraftforge.forge").expect("uid"),
        version: ComponentVersion::new("52.0.1").expect("version"),
        kind: ComponentKind::Loader,
        provenance: ComponentProvenance::new("forge-maven", Some("fixture".to_owned()))
            .expect("provenance"),
    }
}

fn output(integrity: ArtifactIntegrity) -> GeneratedOutput {
    GeneratedOutput {
        id: "processor-000-output-000".to_owned(),
        producer: "processor-000".to_owned(),
        input_identity: "installer=fixture;jar=fixture".to_owned(),
        staging_path: ManagedPath::new("root/libraries/example/generated.jar")
            .expect("staging path"),
        managed_destination: ManagedPath::new("shared/libraries/example/generated.jar")
            .expect("managed path"),
        scope: GeneratedOutputScope::SharedImmutable,
        expected_size: Some(5),
        expected_integrity: integrity,
    }
}

#[test]
fn locally_derived_output_is_published_with_provenance_and_reused() {
    let root = temp_root("reuse");
    let data_root = DataRoot::initialize(&root).expect("data root");
    let work = root.join("work");
    let source = work.join("root/libraries/example/generated.jar");

    fs::create_dir_all(source.parent().expect("parent")).expect("dirs");
    fs::write(&source, b"hello").expect("source");

    let output = output(ArtifactIntegrity::none());
    let component = component();
    let mut embedded = BTreeMap::new();

    embedded.insert("data/client.lzma".to_owned(), "abc123".to_owned());

    verify_and_publish_generated_output(
        &data_root,
        &work,
        &output,
        &component,
        &embedded,
        &CancellationToken::new(),
    )
    .expect("publish");
    assert!(
        reusable_generated_output(
            &data_root,
            &output,
            &component,
            &embedded,
            &CancellationToken::new(),
        )
        .expect("reuse")
    );

    fs::write(
        root.join("shared/libraries/example/generated.jar"),
        b"HELLO",
    )
    .expect("corrupt");
    assert!(
        !reusable_generated_output(
            &data_root,
            &output,
            &component,
            &embedded,
            &CancellationToken::new(),
        )
        .expect("corruption check")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn oversized_generated_output_is_rejected_before_hashing() {
    let root = temp_root("oversized");
    let path = root.join("generated.jar");
    let file = fs::File::create(&path).expect("sparse output");
    file.set_len(MAX_GENERATED_OUTPUT_BYTES + 1)
        .expect("set sparse length");
    drop(file);

    assert_eq!(
        locally_derived_sha256(&path, &CancellationToken::new())
            .expect_err("oversized generated output")
            .code,
        ErrorCode::LoaderProcessorOutputMismatch
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn upstream_hash_mismatch_is_never_published() {
    let root = temp_root("mismatch");
    let data_root = DataRoot::initialize(&root).expect("data root");
    let work = root.join("work");
    let source = work.join("root/libraries/example/generated.jar");

    fs::create_dir_all(source.parent().expect("parent")).expect("dirs");
    fs::write(&source, b"hello").expect("source");

    let integrity = ArtifactIntegrity::none().with_sha1(
        "0000000000000000000000000000000000000000"
            .parse()
            .expect("sha1"),
    );
    let error = verify_and_publish_generated_output(
        &data_root,
        &work,
        &output(integrity),
        &component(),
        &BTreeMap::new(),
        &CancellationToken::new(),
    )
    .expect_err("mismatch");

    assert_eq!(error.code, ErrorCode::LoaderProcessorOutputMismatch);
    assert!(!root.join("shared/libraries/example/generated.jar").exists());

    let _ = fs::remove_dir_all(root);
}
