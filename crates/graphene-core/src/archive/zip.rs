use super::{ArchiveCodecError, ArchiveCodecResult, deflate::inflate_raw_bounded};
use crate::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntry {
    pub name: String,
    pub flags: u16,
    pub method: u16,
    pub crc32: u32,
    pub compressed_size: usize,
    pub uncompressed_size: usize,
    pub local_offset: usize,
    pub is_directory: bool,
    pub is_symlink: bool,
    pub is_regular: bool,
}

pub fn central_entries(
    bytes: &[u8],
    max_entries: usize,
    max_name_bytes: usize,
) -> ArchiveCodecResult<Vec<ZipEntry>> {
    let eocd = find_eocd(bytes)
        .ok_or_else(|| ArchiveCodecError::invalid("ZIP archive has no valid end record"))?;
    if eocd + 22 > bytes.len() {
        return Err(ArchiveCodecError::invalid(
            "ZIP archive end record is truncated",
        ));
    }

    let disk = u16_at(bytes, eocd + 4)?;
    let central_disk = u16_at(bytes, eocd + 6)?;
    let disk_entries = u16_at(bytes, eocd + 8)? as usize;
    let total_entries = u16_at(bytes, eocd + 10)? as usize;
    let central_size = u32_at(bytes, eocd + 12)? as usize;
    let central_offset = u32_at(bytes, eocd + 16)? as usize;
    let comment_len = u16_at(bytes, eocd + 20)? as usize;

    if disk != 0 || central_disk != 0 || disk_entries != total_entries {
        return Err(ArchiveCodecError::invalid(
            "multi-disk ZIP archives are unsupported",
        ));
    }

    if total_entries == u16::MAX as usize
        || central_size == u32::MAX as usize
        || central_offset == u32::MAX as usize
    {
        return Err(ArchiveCodecError::invalid("ZIP64 archives are unsupported"));
    }

    if total_entries > max_entries
        || eocd + 22 + comment_len > bytes.len()
        || central_offset
            .checked_add(central_size)
            .is_none_or(|end| end > bytes.len())
    {
        return Err(ArchiveCodecError::invalid(
            "ZIP archive central directory is out of bounds",
        ));
    }

    let central_end = central_offset + central_size;
    let mut cursor = central_offset;
    let mut entries = Vec::with_capacity(total_entries);

    for _ in 0..total_entries {
        if cursor + 46 > central_end || u32_at(bytes, cursor)? != 0x0201_4b50 {
            return Err(ArchiveCodecError::invalid(
                "ZIP archive central directory entry is malformed",
            ));
        }

        let made_by = u16_at(bytes, cursor + 4)?;
        let flags = u16_at(bytes, cursor + 8)?;
        let method = u16_at(bytes, cursor + 10)?;
        let crc = u32_at(bytes, cursor + 16)?;
        let compressed_size = u32_at(bytes, cursor + 20)? as usize;
        let uncompressed_size = u32_at(bytes, cursor + 24)? as usize;
        let name_len = u16_at(bytes, cursor + 28)? as usize;
        let extra_len = u16_at(bytes, cursor + 30)? as usize;
        let comment_len = u16_at(bytes, cursor + 32)? as usize;
        let external = u32_at(bytes, cursor + 38)?;
        let local_offset = u32_at(bytes, cursor + 42)? as usize;

        if compressed_size == u32::MAX as usize
            || uncompressed_size == u32::MAX as usize
            || local_offset == u32::MAX as usize
        {
            return Err(ArchiveCodecError::invalid(
                "ZIP64 archive entry is unsupported",
            ));
        }

        let record_len = 46usize
            .checked_add(name_len)
            .and_then(|value| value.checked_add(extra_len))
            .and_then(|value| value.checked_add(comment_len))
            .ok_or_else(|| ArchiveCodecError::invalid("ZIP archive entry size overflowed"))?;
        if name_len == 0
            || name_len > max_name_bytes
            || cursor
                .checked_add(record_len)
                .is_none_or(|end| end > central_end)
        {
            return Err(ArchiveCodecError::invalid(
                "ZIP archive entry name or record is invalid",
            ));
        }

        if flags & 0x0001 != 0 {
            return Err(ArchiveCodecError::invalid(
                "encrypted ZIP archive entries are unsupported",
            ));
        }

        let name_bytes = &bytes[cursor + 46..cursor + 46 + name_len];
        let name = std::str::from_utf8(name_bytes)
            .map_err(|_| ArchiveCodecError::invalid("ZIP archive entry name is not UTF-8"))?
            .to_owned();
        let host = made_by >> 8;
        let unix_mode = (external >> 16) as u16;
        let file_type = unix_mode & 0o170000;
        let is_directory = name.ends_with('/') || (host == 3 && file_type == 0o040000);
        let is_symlink = host == 3 && file_type == 0o120000;
        let is_regular =
            !is_directory && !is_symlink && (host != 3 || file_type == 0 || file_type == 0o100000);

        entries.push(ZipEntry {
            name,
            flags,
            method,
            crc32: crc,
            compressed_size,
            uncompressed_size,
            local_offset,
            is_directory,
            is_symlink,
            is_regular,
        });
        cursor += record_len;
    }

    if cursor != central_end {
        return Err(ArchiveCodecError::invalid(
            "ZIP archive central directory length is inconsistent",
        ));
    }

    Ok(entries)
}

pub fn extract_entry(
    bytes: &[u8],
    entry: &ZipEntry,
    maximum_size: usize,
    cancellation: &CancellationToken,
) -> ArchiveCodecResult<Vec<u8>> {
    if entry.uncompressed_size > maximum_size {
        return Err(ArchiveCodecError::invalid(
            "ZIP archive entry exceeds its configured output bound",
        ));
    }

    let cursor = entry.local_offset;
    if cursor + 30 > bytes.len() || u32_at(bytes, cursor)? != 0x0403_4b50 {
        return Err(ArchiveCodecError::invalid(
            "ZIP archive local entry is malformed",
        ));
    }

    let local_flags = u16_at(bytes, cursor + 6)?;
    let method = u16_at(bytes, cursor + 8)?;
    if local_flags & 0x0001 != 0
        || method != entry.method
        || (local_flags & 0x0800) != (entry.flags & 0x0800)
    {
        return Err(ArchiveCodecError::invalid(
            "ZIP archive local/central entry metadata is inconsistent",
        ));
    }

    let name_len = u16_at(bytes, cursor + 26)? as usize;
    let extra_len = u16_at(bytes, cursor + 28)? as usize;
    let data_start = cursor
        .checked_add(30)
        .and_then(|value| value.checked_add(name_len))
        .and_then(|value| value.checked_add(extra_len))
        .ok_or_else(|| ArchiveCodecError::invalid("ZIP archive local entry offset overflowed"))?;
    let data_end = data_start
        .checked_add(entry.compressed_size)
        .ok_or_else(|| ArchiveCodecError::invalid("ZIP archive data range overflowed"))?;
    if data_end > bytes.len() {
        return Err(ArchiveCodecError::invalid(
            "ZIP archive entry data is truncated",
        ));
    }

    let compressed = &bytes[data_start..data_end];
    let output = match entry.method {
        0 => compressed.to_vec(),
        8 => inflate_raw_bounded(
            compressed,
            entry.uncompressed_size,
            maximum_size,
            cancellation,
        )?,
        _ => {
            return Err(ArchiveCodecError::invalid(
                "ZIP archive uses an unsupported compression method",
            ));
        }
    };

    if output.len() != entry.uncompressed_size {
        return Err(ArchiveCodecError::invalid(
            "ZIP archive entry decompressed to an unexpected size",
        ));
    }

    Ok(output)
}

fn find_eocd(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 22 {
        return None;
    }

    let start = bytes.len().saturating_sub(22 + u16::MAX as usize);
    (start..=bytes.len() - 22)
        .rev()
        .find(|&index| bytes.get(index..index + 4) == Some(&[0x50, 0x4b, 0x05, 0x06]))
}

fn u16_at(bytes: &[u8], offset: usize) -> ArchiveCodecResult<u16> {
    let data = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| ArchiveCodecError::invalid("ZIP archive integer is out of bounds"))?;
    Ok(u16::from_le_bytes([data[0], data[1]]))
}

fn u32_at(bytes: &[u8], offset: usize) -> ArchiveCodecResult<u32> {
    let data = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| ArchiveCodecError::invalid("ZIP archive integer is out of bounds"))?;
    Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
}

pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}
