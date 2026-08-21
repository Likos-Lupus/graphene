use graphene::Graphene;
use tempfile::tempdir;

#[tokio::test]
async fn independent_engines_initialize_isolated_data_roots() {
    let first_root = tempdir().expect("first data root");
    let second_root = tempdir().expect("second data root");

    let first = Graphene::builder(first_root.path())
        .build()
        .await
        .expect("first engine");
    let second = Graphene::builder(second_root.path())
        .build()
        .await
        .expect("second engine");

    let first_canonical = first_root
        .path()
        .canonicalize()
        .expect("canonical first root");
    let second_canonical = second_root
        .path()
        .canonicalize()
        .expect("canonical second root");

    assert_eq!(first.data_root(), first_canonical.as_path());
    assert_eq!(second.data_root(), second_canonical.as_path());
    assert_ne!(first.data_root(), second.data_root());
    assert!(first_root.path().join(".graphene-layout.json").is_file());
    assert!(second_root.path().join(".graphene-layout.json").is_file());
}
