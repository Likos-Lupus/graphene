use super::*;

#[test]
fn manifest_main_class_parser_is_bounded_and_handles_continuations() {
    assert_eq!(
        parse_manifest_main_class(
            b"Manifest-Version: 1.0\r\nMain-Class: com.example.\r\n Tool\r\n"
        )
        .expect("main class"),
        "com.example.Tool"
    );
    assert!(parse_manifest_main_class(b"Manifest-Version: 1.0\n").is_err());
    assert!(parse_manifest_main_class(b"Main-Class: bad class\n").is_err());
}

#[test]
fn traversal_names_are_rejected() {
    assert!(validate_entry_name("../evil.dll").is_err());
    assert!(validate_entry_name("/absolute.dll").is_err());
    assert!(validate_entry_name("C:/evil.dll").is_err());
    assert!(validate_entry_name("good/native.dll").is_ok());
}

#[test]
fn bounded_archive_parser_property_inputs_do_not_panic() {
    let mut state = 0xa409_3822_u32;
    for length in 0..512usize {
        let mut bytes = Vec::with_capacity(length);
        for _ in 0..length {
            state = state.wrapping_mul(22_695_477).wrapping_add(1);
            bytes.push((state >> 16) as u8);
        }
        let _ = central_entries(&bytes, MAX_ENTRIES, MAX_NAME_BYTES);
    }

    for value in [
        "a",
        "../x",
        "/x",
        "C:/x",
        "a\\b",
        "a/b/c",
        "META-INF/MANIFEST.MF",
    ] {
        let _ = validate_entry_name(value);
    }
}

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/native-jars")
        .join(name)
}

fn temporary_directory(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "graphene-native-archive-{label}-{}",
        graphene_core::ArtifactId::new()
    ));
    fs::create_dir(&path).expect("create test directory");
    path
}

#[test]
fn frozen_deflated_native_fixture_extracts_without_meta_inf() {
    let root = temporary_directory("valid");
    extract_native_zip(
        &fixture("fixture-native.jar"),
        &root,
        &CancellationToken::new(),
    )
    .expect("extract fixture");

    assert_eq!(
        fs::read(root.join("native/fixture-native.bin")).expect("native bytes"),
        b"graphene-native-fixture\n"
    );
    assert!(!root.join("META-INF/MANIFEST.MF").exists());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn frozen_unsafe_native_archives_are_rejected() {
    for name in ["traversal.jar", "absolute.jar", "symlink.jar"] {
        let root = temporary_directory(name);
        let result = extract_native_zip(&fixture(name), &root, &CancellationToken::new());

        assert!(result.is_err(), "{name} unexpectedly extracted");

        fs::remove_dir_all(root).expect("cleanup");
    }
}

fn managed_fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/managed-java")
        .join(name)
}

#[test]
fn frozen_managed_zip_and_tar_gz_fixtures_extract() {
    for (name, tar_gz) in [("valid.zip", false), ("valid.tar.gz", true)] {
        let root = temporary_directory(name);

        let result = if tar_gz {
            extract_managed_tar_gz(&managed_fixture(name), &root, &CancellationToken::new())
        } else {
            extract_managed_zip(&managed_fixture(name), &root, &CancellationToken::new())
        };

        result.expect("valid managed archive");

        assert_eq!(
            fs::read(root.join("runtime/bin/java")).expect("java fixture"),
            b"fixture-java\n"
        );

        fs::remove_dir_all(root).expect("cleanup");
    }
}

#[test]
fn frozen_hostile_managed_archives_never_escape_or_commit() {
    for (name, tar_gz) in [
        ("traversal.zip", false),
        ("absolute.zip", false),
        ("backslash.zip", false),
        ("symlink.zip", false),
        ("oversized-entry.zip", false),
        ("bomb-policy.zip", false),
        ("corrupt.zip", false),
        ("traversal.tar.gz", true),
        ("absolute.tar.gz", true),
        ("symlink.tar.gz", true),
        ("hardlink.tar.gz", true),
        ("oversized-entry.tar.gz", true),
        ("bomb-policy.tar.gz", true),
        ("corrupt.tar.gz", true),
    ] {
        let outer = temporary_directory(&format!("managed-hostile-{name}"));
        let root = outer.join("extract");
        fs::create_dir(&root).expect("create extraction root");

        let result = if tar_gz {
            extract_managed_tar_gz(&managed_fixture(name), &root, &CancellationToken::new())
        } else {
            extract_managed_zip(&managed_fixture(name), &root, &CancellationToken::new())
        };

        assert!(result.is_err(), "{name} unexpectedly extracted");
        assert!(!outer.join("escape.txt").exists());

        fs::remove_dir_all(outer).expect("cleanup");
    }
}
