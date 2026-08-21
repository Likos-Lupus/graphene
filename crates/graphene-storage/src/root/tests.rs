use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("graphene-storage-{name}-{unique}"))
}

#[test]
fn initializes_empty_root_idempotently() {
    let path = temp_root("init");
    let first = DataRoot::initialize(&path).expect("first init");
    let second = DataRoot::initialize(&path).expect("second init");
    assert_eq!(first.path(), second.path());
    assert!(first.layout_marker_path().is_file());
    for relative in DIRECTORIES {
        assert!(first.path().join(relative).is_dir(), "missing {relative}");
    }
    let _ = fs::remove_dir_all(path);
}

#[test]
fn initialization_preserves_unknown_files() {
    let path = temp_root("unknown-files");
    fs::create_dir_all(&path).expect("root");
    let unknown = path.join("user-note.txt");
    fs::write(&unknown, b"preserve me").expect("unknown file");

    DataRoot::initialize(&path).expect("initialize");
    assert_eq!(
        fs::read(&unknown).expect("unknown survives"),
        b"preserve me"
    );
    let _ = fs::remove_dir_all(path);
}

#[test]
fn invalid_layout_marker_is_rejected() {
    let path = temp_root("marker");
    let root = DataRoot::initialize(&path).expect("init");
    fs::write(root.layout_marker_path(), br#"{"layout_version":999}"#).expect("marker");
    let error = DataRoot::initialize(&path).expect_err("must reject layout");
    assert_eq!(error.code, ErrorCode::StorageLayoutInvalid);
    let _ = fs::remove_dir_all(path);
}

#[test]
fn cache_path_is_deterministic_and_distinct_from_temp() {
    let path = temp_root("cache");
    let root = DataRoot::initialize(&path).expect("init");
    let sha256 = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        .parse()
        .expect("sha256");
    let integrity = ArtifactIntegrity::none().with_sha256(sha256);
    let first = root.cache_address(&integrity).expect("cache");
    let second = root.cache_address(&integrity).expect("cache");
    assert_eq!(first.path(), second.path());
    let temp = root
        .download_temp_path(ArtifactId::new(), OperationId::new())
        .expect("temp path");
    assert_ne!(temp, first.path());
    assert!(temp.starts_with(root.path().join("cache/downloads/temporary")));
    assert!(first.path().starts_with(root.path().join("cache/objects")));
    let _ = fs::remove_dir_all(path);
}

#[cfg(unix)]
#[test]
fn layout_directory_symlink_escape_is_rejected() {
    use std::os::unix::fs::symlink;

    let path = temp_root("symlink-root");
    let outside = temp_root("symlink-outside");
    fs::create_dir_all(&outside).expect("outside");
    let root = DataRoot::initialize(&path).expect("initial root");
    let objects = root.path().join("cache/objects");
    fs::remove_dir(&objects).expect("remove objects");
    symlink(&outside, &objects).expect("symlink");

    let error = DataRoot::initialize(&path).expect_err("escape must fail");
    assert_eq!(error.code, ErrorCode::DataRootInvalid);
    let _ = fs::remove_dir_all(path);
    let _ = fs::remove_dir_all(outside);
}

#[test]
fn commit_rejects_non_file_temporary_source_without_destination_change() {
    let path = temp_root("commit-failure");
    let root = DataRoot::initialize(&path).expect("init");
    let sha256 = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        .parse()
        .expect("sha256");
    let destination = root
        .cache_address(&ArtifactIntegrity::none().with_sha256(sha256))
        .expect("cache")
        .path()
        .to_path_buf();
    fs::create_dir_all(destination.parent().expect("destination parent"))
        .expect("destination parent");
    fs::write(&destination, b"previous-valid").expect("previous destination");
    let temporary = root
        .download_temp_path(ArtifactId::new(), OperationId::new())
        .expect("temp path");
    fs::create_dir(&temporary).expect("directory-shaped temp");

    let error = root
        .commit_verified(&temporary, &destination)
        .expect_err("non-file commit source must fail");
    assert_eq!(error.code, ErrorCode::CacheCommitFailed);
    assert_eq!(
        fs::read(&destination).expect("previous survives"),
        b"previous-valid"
    );
    let _ = fs::remove_dir_all(path);
}

#[cfg(unix)]
#[test]
fn cache_root_symlink_swap_is_rejected_after_initialization() {
    use std::os::unix::fs::symlink;

    let path = temp_root("symlink-swap");
    let outside = temp_root("symlink-swap-outside");
    fs::create_dir_all(&outside).expect("outside");
    let root = DataRoot::initialize(&path).expect("root");
    let objects = root.path().join("cache/objects");
    fs::remove_dir(&objects).expect("remove objects");
    symlink(&outside, &objects).expect("symlink objects");
    let candidate = objects.join("sha256/aa/aa");

    let error = root
        .committed_file_exists(&candidate)
        .expect_err("swapped root must fail");
    assert_eq!(error.code, ErrorCode::DataRootInvalid);
    let _ = fs::remove_dir_all(path);
    let _ = fs::remove_dir_all(outside);
}

#[cfg(unix)]
#[test]
fn commit_rejects_nested_object_symlink_without_writing_outside_root() {
    use std::os::unix::fs::symlink;

    let path = temp_root("nested-object-symlink");
    let outside = temp_root("nested-object-symlink-outside");
    fs::create_dir_all(&outside).expect("outside");
    let root = DataRoot::initialize(&path).expect("root");
    symlink(&outside, root.path().join("cache/objects/sha256")).expect("symlink algorithm");

    let sha256 = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        .parse()
        .expect("sha256");
    let destination = root
        .cache_address(&ArtifactIntegrity::none().with_sha256(sha256))
        .expect("cache address")
        .path()
        .to_path_buf();
    let temporary = root
        .download_temp_path(ArtifactId::new(), OperationId::new())
        .expect("temp path");
    fs::write(&temporary, b"verified-content").expect("temporary content");

    let error = root
        .commit_verified(&temporary, &destination)
        .expect_err("nested symlink must fail");
    assert_eq!(error.code, ErrorCode::DataRootInvalid);
    assert!(!outside.join("ba").exists());
    assert!(temporary.is_file());
    let _ = fs::remove_dir_all(path);
    let _ = fs::remove_dir_all(outside);
}

#[cfg(unix)]
#[test]
fn temporary_directory_symlink_swap_is_rejected_without_writing_outside_root() {
    use std::os::unix::fs::symlink;

    let path = temp_root("temporary-symlink-swap");
    let outside = temp_root("temporary-symlink-swap-outside");
    fs::create_dir_all(&outside).expect("outside");
    let root = DataRoot::initialize(&path).expect("root");
    let temporary = root.path().join("cache/downloads/temporary");
    fs::remove_dir(&temporary).expect("remove temporary directory");
    symlink(&outside, &temporary).expect("symlink temporary directory");

    let error = root
        .download_temp_path(ArtifactId::new(), OperationId::new())
        .expect_err("swapped temporary directory must fail");
    assert_eq!(error.code, ErrorCode::DataRootInvalid);
    assert_eq!(fs::read_dir(&outside).expect("outside listing").count(), 0);
    let _ = fs::remove_dir_all(path);
    let _ = fs::remove_dir_all(outside);
}

#[test]
fn independent_roots_do_not_collide() {
    let path_a = temp_root("a");
    let path_b = temp_root("b");
    let a = DataRoot::initialize(&path_a).expect("a");
    let b = DataRoot::initialize(&path_b).expect("b");
    assert_ne!(a.path(), b.path());
    assert!(!a.path().starts_with(b.path()));
    assert!(!b.path().starts_with(a.path()));
    let _ = fs::remove_dir_all(path_a);
    let _ = fs::remove_dir_all(path_b);
}
