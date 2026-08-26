use graphene_core::CancellationToken;
use graphene_core::archive::{ArchiveFile, DeterministicZipWriter};
use std::io::Cursor;

fn fixture_path(name: &str) -> String {
    format!(
        "{}/tests/fixtures/archive/{name}",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn open_fixture(
    name: &str,
) -> Result<ArchiveFile<std::io::BufReader<std::fs::File>>, graphene_core::archive::ArchiveCodecError>
{
    let file = std::fs::File::open(fixture_path(name))
        .map_err(|_| graphene_core::archive::ArchiveCodecError::io("fixture is missing"))?;
    let reader = std::io::BufReader::new(file);
    ArchiveFile::open(reader, 10_000, 1_024, &CancellationToken::new())
}

#[test]
fn reader_lists_central_directory_entries() {
    let archive = open_fixture("stored.zip").expect("valid archive");
    let names: Vec<&str> = archive.entries().iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["hello.txt", "nested/dir/file.txt", "empty.bin"]);
    assert!(archive.entry("hello.txt").is_some());
    assert!(archive.entry("missing.txt").is_none());
}

#[test]
fn reader_streams_stored_and_deflated_payloads() {
    let mut archive = open_fixture("deflated.zip").expect("valid archive");
    let big = archive.entry("big.bin").expect("big entry").clone();
    assert_eq!(
        big.uncompressed_size,
        "graphene modpack payload repeated ".len() * 8000 + 256 * 40
    );

    let mut buffer = Vec::new();
    let written = archive
        .stream_entry_to(
            &big,
            big.uncompressed_size as u64,
            &mut Cursor::new(&mut buffer),
            &CancellationToken::new(),
        )
        .expect("streaming succeeds");
    assert_eq!(written as usize, big.uncompressed_size);
    assert!(buffer.starts_with(b"graphene modpack payload repeated"));

    let small = archive.entry("small.txt").expect("small entry").clone();
    let bytes = archive
        .read_entry(&small, 1024, &CancellationToken::new())
        .expect("small entry reads");
    assert_eq!(bytes, b"tiny");
}

#[test]
fn reader_handles_unicode_names_and_mixed_methods() {
    let mut archive = open_fixture("mixed.zip").expect("valid archive");
    let unicode = archive
        .entry("data/\u{fc}n\u{ef}code.txt")
        .expect("unicode entry")
        .clone();
    let bytes = archive
        .read_entry(&unicode, 1024, &CancellationToken::new())
        .expect("unicode entry reads");
    assert_eq!(bytes, b"unicode content");

    let stored = archive
        .entry("raw/stored.dat")
        .expect("stored entry")
        .clone();
    assert_eq!(stored.method, 0);
    let bytes = archive
        .read_entry(&stored, 1024, &CancellationToken::new())
        .expect("stored entry reads");
    assert_eq!(bytes, [0x00, 0x01, 0x02, 0x03]);
}

#[test]
fn reader_enforces_per_entry_output_bound() {
    let mut archive = open_fixture("stored.zip").expect("valid archive");
    let hello = archive.entry("hello.txt").expect("hello").clone();
    let error = archive
        .stream_entry_to(&hello, 4, &mut Vec::new(), &CancellationToken::new())
        .expect_err("bound must fail closed");
    assert!(!error.is_cancelled());
}

#[test]
fn reader_rejects_crc_corruption() {
    let mut archive = open_fixture("corrupt_crc.zip").expect("archive opens");
    let entry = archive.entries()[0].clone();
    let error = archive
        .read_entry(&entry, 1_048_576, &CancellationToken::new())
        .expect_err("corrupted payload must fail its CRC check");
    assert!(!error.is_cancelled());
}

#[test]
fn reader_rejects_truncated_archive() {
    let result = open_fixture("truncated.zip");
    assert!(result.is_err(), "truncated archives must be rejected");
}

#[test]
fn reader_classifies_symlink_entries_as_special() {
    let archive = open_fixture("symlink.zip").expect("valid archive");
    let link = archive.entry("link.txt").expect("link entry");
    assert!(link.is_symlink);
    assert!(!link.is_regular);
}

#[test]
fn reader_rejects_encrypted_flag() {
    let result = open_fixture("encrypted_flag.zip");
    assert!(result.is_err(), "encrypted entries must be rejected");
}

#[test]
fn reader_rejects_unsupported_compression_method() {
    let mut archive = open_fixture("unsupported_method.zip").expect("archive opens");
    let entry = archive.entries()[0].clone();
    let error = archive
        .read_entry(&entry, 1024, &CancellationToken::new())
        .expect_err("unsupported compression must fail closed");
    assert!(!error.is_cancelled());
}

#[test]
fn writer_output_is_deterministic_and_round_trips() {
    let build = || {
        let mut writer = DeterministicZipWriter::new(Cursor::new(Vec::new()));
        writer.add_entry("b.txt", b"second").expect("entry b");
        writer
            .add_entry("a/nested.txt", b"first payload")
            .expect("entry a");
        writer.finish().expect("finish");
        writer.into_inner().expect("finalized").into_inner()
    };

    let first = build();
    let second = build();
    assert_eq!(
        first, second,
        "identical entry sequences must be byte-identical"
    );

    let reader = ArchiveFile::open(
        Cursor::new(first.clone()),
        128,
        1_024,
        &CancellationToken::new(),
    )
    .expect("written archive parses");
    let names: Vec<&str> = reader.entries().iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["b.txt", "a/nested.txt"]);

    let mut reader = ArchiveFile::open(Cursor::new(first), 128, 1_024, &CancellationToken::new())
        .expect("parse");
    let nested = reader.entry("a/nested.txt").expect("nested entry").clone();
    let bytes = reader
        .read_entry(&nested, 1024, &CancellationToken::new())
        .expect("round-trip read");
    assert_eq!(bytes, b"first payload");
}

#[test]
fn writer_rejects_duplicate_and_invalid_names() {
    let mut writer = DeterministicZipWriter::new(Cursor::new(Vec::new()));
    writer.add_entry("same.txt", b"one").expect("first entry");
    assert!(writer.add_entry("same.txt", b"two").is_err());
    assert!(writer.add_entry("", b"empty").is_err());
    assert!(writer.add_entry("back\\slash.txt", b"x").is_err());
}

#[test]
fn streaming_respects_cancellation() {
    let token = CancellationToken::new();
    let mut archive = open_fixture("deflated.zip").expect("valid archive");
    let big = archive.entry("big.bin").expect("big entry").clone();
    token.cancel();
    let error = archive
        .stream_entry_to(&big, big.uncompressed_size as u64, &mut Vec::new(), &token)
        .expect_err("cancelled stream must fail");
    assert!(error.is_cancelled());
}
