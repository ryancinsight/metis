//! Bounded USTAR reading for Linux package installation.

use crate::{Result, manifest};
use std::{
    fs,
    path::{Component, Path},
};

const BLOCK_BYTES: usize = 512;
const MAX_FILES: usize = 4096;
// The package payload is capped at 1 GiB; this additional bound covers USTAR
// headers and the two end blocks without permitting an unbounded read.
const MAX_ARCHIVE_BYTES: u64 = manifest::PAYLOAD_LIMIT + 8 * 1024 * 1024;

pub(super) struct ArchiveEntry {
    pub(super) path: String,
    pub(super) mode: u32,
    pub(super) bytes: Vec<u8>,
}

pub(super) fn read_archive(path: &Path) -> Result<Vec<ArchiveEntry>> {
    let size = fs::metadata(path)?.len();
    if size > MAX_ARCHIVE_BYTES {
        return Err("Linux archive exceeds the bounded installation size".into());
    }
    let mut file = fs::File::open(path)?;
    let mut bytes = Vec::with_capacity(usize::try_from(size).map_err(|_| "archive is too large")?);
    std::io::Read::read_to_end(&mut file, &mut bytes)?;
    let mut entries = Vec::new();
    let mut offset = 0_usize;
    let mut zero_blocks = 0_u8;
    let mut total_payload = 0_u64;
    while offset
        .checked_add(BLOCK_BYTES)
        .is_some_and(|end| end <= bytes.len())
    {
        let header = &bytes[offset..offset + BLOCK_BYTES];
        offset += BLOCK_BYTES;
        if header.iter().all(|byte| *byte == 0) {
            zero_blocks = zero_blocks.saturating_add(1);
            if zero_blocks == 2 {
                break;
            }
            continue;
        }
        zero_blocks = 0;
        validate_checksum(header)?;
        if &header[257..263] != b"ustar\0" || header[156] != b'0' {
            return Err("Linux archive contains a non-USTAR or non-regular entry".into());
        }
        let name = field_text(&header[0..100])?;
        let prefix = field_text(&header[345..500])?;
        let path = if prefix.is_empty() {
            name
        } else if name.is_empty() {
            return Err("USTAR entry has an empty name".into());
        } else {
            format!("{prefix}/{name}")
        };
        validate_archive_path(&path)?;
        let mode = parse_octal(&header[100..108])?;
        let size = parse_octal(&header[124..136])?;
        total_payload = total_payload
            .checked_add(size)
            .ok_or("Linux archive payload size overflow")?;
        if total_payload > manifest::PAYLOAD_LIMIT {
            return Err("Linux archive payload exceeds the bounded installation size".into());
        }
        let payload_size = usize::try_from(size).map_err(|_| "archive entry is too large")?;
        let end = offset
            .checked_add(payload_size)
            .ok_or("Linux archive offset overflow")?;
        if end > bytes.len() {
            return Err("Linux archive entry is truncated".into());
        }
        entries.push(ArchiveEntry {
            path,
            mode: u32::try_from(mode).map_err(|_| "USTAR mode is too large")?,
            bytes: bytes[offset..end].to_vec(),
        });
        if entries.len() > MAX_FILES {
            return Err("Linux archive contains too many files".into());
        }
        offset = end
            .checked_add((BLOCK_BYTES - (payload_size % BLOCK_BYTES)) % BLOCK_BYTES)
            .ok_or("Linux archive padding overflow")?;
        if offset > bytes.len() {
            return Err("Linux archive padding is truncated".into());
        }
    }
    if zero_blocks != 2 {
        return Err("Linux archive is missing its USTAR end blocks".into());
    }
    Ok(entries)
}

fn validate_archive_path(path: &str) -> Result<()> {
    if !path.starts_with("usr/") || path.contains('\\') {
        return Err("Linux archive path is outside the FHS usr prefix".into());
    }
    safe_relative(path).map(|_| ())
}

fn validate_checksum(header: &[u8]) -> Result<()> {
    let expected = parse_octal(&header[148..156])?;
    let actual = header
        .iter()
        .enumerate()
        .map(|(index, byte)| {
            if (148..156).contains(&index) {
                u32::from(b' ')
            } else {
                u32::from(*byte)
            }
        })
        .sum::<u32>();
    if u64::from(actual) != expected {
        return Err("Linux archive header checksum is invalid".into());
    }
    Ok(())
}

fn field_text(field: &[u8]) -> Result<String> {
    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());
    let field = &field[..end];
    let field = std::str::from_utf8(field).map_err(|_| "USTAR field is not UTF-8")?;
    Ok(field.trim_end_matches(' ').to_owned())
}

fn parse_octal(field: &[u8]) -> Result<u64> {
    let mut value = 0_u64;
    let mut digits = 0_u8;
    for byte in field {
        if matches!(*byte, 0 | b' ') {
            continue;
        }
        if !(b'0'..=b'7').contains(byte) {
            return Err("USTAR numeric field is not octal".into());
        }
        value = value
            .checked_mul(8)
            .and_then(|value| value.checked_add(u64::from(*byte - b'0')))
            .ok_or("USTAR numeric field overflows")?;
        digits = digits.saturating_add(1);
    }
    if digits == 0 {
        return Err("USTAR numeric field is empty".into());
    }
    Ok(value)
}

pub(super) fn safe_relative(value: &str) -> Result<&str> {
    manifest::relative(value)?;
    if Path::new(value)
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("installation path must remain relative".into());
    }
    Ok(value)
}
