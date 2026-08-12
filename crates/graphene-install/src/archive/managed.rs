use super::{
    MAX_ARCHIVE_BYTES, MAX_ENTRIES, MAX_ENTRY_BYTES, MAX_EXPANSION_RATIO, MAX_TOTAL_BYTES,
    archive_error, checkpoint, native_codec_error, validate_entry_name,
};
use graphene_core::{
    CancellationToken, Result,
    archive::{crc32, inflate_raw_bounded},
};
use graphene_platform::{ManagedRelativePath, ensure_managed_directory};
use std::{fs, io::Write, path::Path};

pub fn extract_tar_gz(
    archive: &Path,
    destination: &Path,
    cancellation: &CancellationToken,
) -> Result<()> {
    let metadata = fs::symlink_metadata(archive).map_err(|source| {
        archive_error("failed to inspect managed runtime archive").with_source(source)
    })?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_ARCHIVE_BYTES
    {
        return Err(archive_error(
            "managed runtime archive is not a bounded ordinary file",
        ));
    }

    let bytes = fs::read(archive).map_err(|source| {
        archive_error("failed to read managed runtime archive").with_source(source)
    })?;
    let tar = inflate_gzip(&bytes, cancellation)?;
    extract_tar(&tar, destination, cancellation)
}

fn inflate_gzip(bytes: &[u8], cancellation: &CancellationToken) -> Result<Vec<u8>> {
    if bytes.len() < 18 || bytes[0] != 0x1f || bytes[1] != 0x8b || bytes[2] != 8 {
        return Err(archive_error("managed runtime gzip header is invalid"));
    }

    let flags = bytes[3];

    if flags & 0xe0 != 0 {
        return Err(archive_error(
            "managed runtime gzip header has reserved flags",
        ));
    }

    let mut cursor = 10usize;

    if flags & 0x04 != 0 {
        let length = u16_le(bytes, cursor)? as usize;
        cursor = cursor
            .checked_add(2 + length)
            .ok_or_else(|| archive_error("gzip extra field overflowed"))?;
    }
    if flags & 0x08 != 0 {
        cursor = scan_zero(bytes, cursor)?;
    }
    if flags & 0x10 != 0 {
        cursor = scan_zero(bytes, cursor)?;
    }
    if flags & 0x02 != 0 {
        cursor = cursor
            .checked_add(2)
            .ok_or_else(|| archive_error("gzip header CRC overflowed"))?;
    }

    if cursor > bytes.len().saturating_sub(8) {
        return Err(archive_error("managed runtime gzip header is truncated"));
    }

    let footer = bytes.len() - 8;
    let expected_crc = u32::from_le_bytes(
        bytes[footer..footer + 4]
            .try_into()
            .expect("fixed footer slice"),
    );
    let expected_size =
        u32::from_le_bytes(bytes[footer + 4..].try_into().expect("fixed footer slice")) as usize;

    if expected_size > MAX_TOTAL_BYTES {
        return Err(archive_error(
            "managed runtime gzip expansion exceeds the total limit",
        ));
    }

    let compressed = &bytes[cursor..footer];

    if expected_size > 0
        && (compressed.is_empty()
            || expected_size > compressed.len().saturating_mul(MAX_EXPANSION_RATIO))
    {
        return Err(archive_error(
            "managed runtime gzip has an implausible expansion ratio",
        ));
    }

    let output = inflate_raw_bounded(compressed, expected_size, MAX_TOTAL_BYTES, cancellation)
        .map_err(native_codec_error)?;

    if output.len() != expected_size || crc32(&output) != expected_crc {
        return Err(archive_error(
            "managed runtime gzip integrity footer does not match",
        ));
    }

    Ok(output)
}

fn extract_tar(bytes: &[u8], destination: &Path, cancellation: &CancellationToken) -> Result<()> {
    let mut cursor = 0usize;
    let mut entries = 0usize;
    let mut expanded = 0usize;

    while cursor < bytes.len() {
        checkpoint(cancellation)?;
        if cursor.checked_add(512).is_none_or(|end| end > bytes.len()) {
            return Err(archive_error("managed runtime tar header is truncated"));
        }

        let header = &bytes[cursor..cursor + 512];
        if header.iter().all(|byte| *byte == 0) {
            return Ok(());
        }

        validate_tar_checksum(header)?;
        entries += 1;
        if entries > MAX_ENTRIES {
            return Err(archive_error(
                "managed runtime archive contains too many entries",
            ));
        }

        let name = tar_path(header)?;
        let relative = validate_entry_name(&name)?;
        let size = parse_octal(&header[124..136])?;

        if size > MAX_ENTRY_BYTES {
            return Err(archive_error(
                "managed runtime archive entry exceeds the per-entry size limit",
            ));
        }

        expanded = expanded
            .checked_add(size)
            .ok_or_else(|| archive_error("managed runtime archive size accounting overflowed"))?;
        if expanded > MAX_TOTAL_BYTES {
            return Err(archive_error(
                "managed runtime archive exceeds the total expansion limit",
            ));
        }

        let typeflag = header[156];
        cursor += 512;

        let data_end = cursor
            .checked_add(size)
            .ok_or_else(|| archive_error("managed runtime tar entry size overflowed"))?;
        if data_end > bytes.len() {
            return Err(archive_error("managed runtime tar entry data is truncated"));
        }

        match typeflag {
            0 | b'0' => write_regular(destination, &relative, &bytes[cursor..data_end])?,
            b'5' => {
                ensure_managed_directory(destination, &relative)?;
            }
            b'1' | b'2' => {
                return Err(archive_error(
                    "managed runtime archive contains a link entry",
                ));
            }
            _ => {
                return Err(archive_error(
                    "managed runtime archive contains an unsupported special entry",
                ));
            }
        }

        let padded = size
            .checked_add(511)
            .ok_or_else(|| archive_error("managed runtime tar padding overflowed"))?
            / 512
            * 512;
        cursor = cursor
            .checked_add(padded)
            .ok_or_else(|| archive_error("managed runtime tar cursor overflowed"))?;

        if cursor > bytes.len() {
            return Err(archive_error("managed runtime tar padding is truncated"));
        }
    }

    Ok(())
}

fn write_regular(root: &Path, relative: &ManagedRelativePath, data: &[u8]) -> Result<()> {
    let parent = relative
        .as_path()
        .parent()
        .unwrap_or_else(|| Path::new("."));
    if parent != Path::new("") && parent != Path::new(".") {
        ensure_managed_directory(root, &ManagedRelativePath::new(parent)?)?;
    }

    let target = relative.under(root);
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&target)
        .map_err(|source| {
            archive_error("failed to create managed runtime archive file").with_source(source)
        })?;
    file.write_all(data)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .map_err(|source| {
            archive_error("failed to write managed runtime archive file").with_source(source)
        })
}

fn tar_path(header: &[u8]) -> Result<String> {
    let name = field_string(&header[0..100])?;
    let prefix = field_string(&header[345..500])?;

    if name.is_empty() {
        return Err(archive_error("managed runtime tar entry has an empty name"));
    }

    Ok(if prefix.is_empty() {
        name
    } else {
        format!("{prefix}/{name}")
    })
}

fn field_string(field: &[u8]) -> Result<String> {
    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());
    std::str::from_utf8(&field[..end])
        .map(str::to_owned)
        .map_err(|source| {
            archive_error("managed runtime tar path is not UTF-8").with_source(source)
        })
}

fn parse_octal(field: &[u8]) -> Result<usize> {
    let text = std::str::from_utf8(field).map_err(|source| {
        archive_error("managed runtime tar numeric field is invalid").with_source(source)
    })?;
    let trimmed = text.trim_matches(|ch| ch == '\0' || ch == ' ');

    if trimmed.is_empty() {
        return Ok(0);
    }

    if !trimmed.bytes().all(|byte| (b'0'..=b'7').contains(&byte)) {
        return Err(archive_error(
            "managed runtime tar numeric field is not octal",
        ));
    }

    usize::from_str_radix(trimmed, 8).map_err(|source| {
        archive_error("managed runtime tar numeric field overflowed").with_source(source)
    })
}

fn validate_tar_checksum(header: &[u8]) -> Result<()> {
    let expected = parse_octal(&header[148..156])?;
    let actual = header
        .iter()
        .enumerate()
        .map(|(index, byte)| {
            if (148..156).contains(&index) {
                usize::from(b' ')
            } else {
                usize::from(*byte)
            }
        })
        .sum::<usize>();

    if expected != actual {
        return Err(archive_error(
            "managed runtime tar header checksum does not match",
        ));
    }

    Ok(())
}

fn scan_zero(bytes: &[u8], mut cursor: usize) -> Result<usize> {
    while cursor < bytes.len() {
        if bytes[cursor] == 0 {
            return Ok(cursor + 1);
        }

        cursor += 1;
        if cursor > 8192 {
            return Err(archive_error(
                "managed runtime gzip header string exceeds its bound",
            ));
        }
    }

    Err(archive_error(
        "managed runtime gzip header string is truncated",
    ))
}

fn u16_le(bytes: &[u8], offset: usize) -> Result<u16> {
    let data = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| archive_error("managed runtime gzip extra length is truncated"))?;
    Ok(u16::from_le_bytes([data[0], data[1]]))
}
