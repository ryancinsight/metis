//! Bounded PNG-in-ICO validation for native application shell branding.
//!
//! The installer consumes an ICO stream rather than a browser asset. Restricting
//! the accepted entries to complete PNG chunks keeps decoding out of the trusted
//! packaging path while still rejecting malformed, overlapping or mismatched
//! images before Windows Installer persistence.
use crate::{Result, manifest};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub(crate) const ICON_LIMIT: u64 = 1024 * 1024;

const HEADER_SIZE: usize = 6;
const ENTRY_SIZE: usize = 16;
const ENTRY_LIMIT: usize = 16;
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

pub(crate) fn source(root: &Path, relative_path: &str) -> Result<PathBuf> {
    let path = manifest::source(root, relative_path)?;
    validate_file(&path)?;
    Ok(path)
}

pub(crate) fn validate_file(path: &Path) -> Result<()> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(ICON_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len())? > ICON_LIMIT {
        return Err("application icon exceeds the 1 MiB budget".into());
    }
    validate(&bytes)
}

pub(crate) fn validate(bytes: &[u8]) -> Result<()> {
    if u64::try_from(bytes.len())? > ICON_LIMIT {
        return Err("application icon exceeds the 1 MiB budget".into());
    }
    let header = bytes.get(..HEADER_SIZE).ok_or("ICO header is truncated")?;
    if u16::from_le_bytes([header[0], header[1]]) != 0
        || u16::from_le_bytes([header[2], header[3]]) != 1
    {
        return Err("ICO header must identify an icon resource".into());
    }
    let count = usize::from(u16::from_le_bytes([header[4], header[5]]));
    if !(1..=ENTRY_LIMIT).contains(&count) {
        return Err("ICO must contain 1..=16 image entries".into());
    }
    let table_size = HEADER_SIZE
        .checked_add(
            count
                .checked_mul(ENTRY_SIZE)
                .ok_or("ICO entry table overflows")?,
        )
        .ok_or("ICO entry table overflows")?;
    if bytes.len() < table_size {
        return Err("ICO entry table is truncated".into());
    }

    let mut ranges = [(0_u64, 0_u64); ENTRY_LIMIT];
    for index in 0..count {
        let start = HEADER_SIZE + index * ENTRY_SIZE;
        let entry = bytes
            .get(start..start + ENTRY_SIZE)
            .ok_or("ICO entry is truncated")?;
        let width = dimension(entry[0]);
        let height = dimension(entry[1]);
        if entry[3] != 0 {
            return Err("ICO entry reserved byte must be zero".into());
        }
        let payload_size = u64::from(little_endian(
            entry.get(8..12).ok_or("ICO payload size is truncated")?,
        )?);
        let payload_offset = u64::from(little_endian(
            entry.get(12..16).ok_or("ICO payload offset is truncated")?,
        )?);
        let table_size = u64::try_from(table_size)?;
        let payload_end = payload_offset
            .checked_add(payload_size)
            .ok_or("ICO image range overflows")?;
        if payload_size == 0
            || payload_offset < table_size
            || payload_end > u64::try_from(bytes.len())?
        {
            return Err("ICO image range is outside the file".into());
        }
        for &(previous_start, previous_end) in ranges.iter().take(index) {
            if payload_offset < previous_end && previous_start < payload_end {
                return Err("ICO image ranges overlap".into());
            }
        }
        ranges[index] = (payload_offset, payload_end);
        let payload = bytes
            .get(usize::try_from(payload_offset)?..usize::try_from(payload_end)?)
            .ok_or("ICO image range is not addressable")?;
        let (png_width, png_height) = png_dimensions(payload)?;
        if png_width != width || png_height != height {
            return Err("ICO entry dimensions differ from its PNG image".into());
        }
    }
    Ok(())
}

fn dimension(value: u8) -> u32 {
    if value == 0 { 256 } else { u32::from(value) }
}

fn png_dimensions(bytes: &[u8]) -> Result<(u32, u32)> {
    if bytes.get(..PNG_SIGNATURE.len()) != Some(PNG_SIGNATURE.as_slice()) {
        return Err("ICO images must use PNG encoding".into());
    }
    let mut cursor = PNG_SIGNATURE.len();
    let mut dimensions = None;
    let mut saw_data = false;
    loop {
        let chunk_header = bytes
            .get(cursor..cursor.checked_add(8).ok_or("PNG chunk offset overflows")?)
            .ok_or("PNG chunk header is truncated")?;
        let length = usize::try_from(big_endian(
            chunk_header
                .get(..4)
                .ok_or("PNG chunk length is truncated")?,
        )?)?;
        let data_start = cursor.checked_add(8).ok_or("PNG data offset overflows")?;
        let data_end = data_start
            .checked_add(length)
            .ok_or("PNG chunk length overflows")?;
        let chunk_end = data_end.checked_add(4).ok_or("PNG CRC offset overflows")?;
        if chunk_end > bytes.len() {
            return Err("PNG chunk is truncated".into());
        }
        let chunk_type = bytes
            .get(cursor + 4..cursor + 8)
            .ok_or("PNG chunk type is truncated")?;
        let crc = big_endian(
            bytes
                .get(data_end..chunk_end)
                .ok_or("PNG CRC is truncated")?,
        )?;
        if metis_core::crc32(&bytes[cursor + 4..data_end]) != crc {
            return Err("PNG chunk CRC does not match its data".into());
        }
        if cursor == PNG_SIGNATURE.len() {
            if chunk_type != b"IHDR" || length != 13 {
                return Err("PNG must begin with a 13-byte IHDR chunk".into());
            }
            let ihdr = bytes
                .get(data_start..data_end)
                .ok_or("PNG IHDR is truncated")?;
            let width = big_endian(ihdr.get(..4).ok_or("PNG width is truncated")?)?;
            let height = big_endian(ihdr.get(4..8).ok_or("PNG height is truncated")?)?;
            if !(1..=256).contains(&width) || !(1..=256).contains(&height) {
                return Err("PNG icon dimensions must be 1..=256".into());
            }
            dimensions = Some((width, height));
        } else if chunk_type == b"IHDR" {
            return Err("PNG contains a duplicate IHDR chunk".into());
        }
        if chunk_type == b"IDAT" {
            saw_data = true;
        }
        if chunk_type == b"IEND" {
            if length != 0 || chunk_end != bytes.len() || !saw_data {
                return Err("PNG IEND chunk is invalid or not final".into());
            }
            return dimensions.ok_or_else(|| "PNG is missing its IHDR chunk".into());
        }
        cursor = chunk_end;
    }
}

fn little_endian(bytes: &[u8]) -> Result<u32> {
    Ok(u32::from_le_bytes(bytes.try_into()?))
}

fn big_endian(bytes: &[u8]) -> Result<u32> {
    Ok(u32::from_be_bytes(bytes.try_into()?))
}

#[cfg(test)]
mod tests {
    use super::{PNG_SIGNATURE, validate};

    const PROJECT_MARK: &[u8] =
        include_bytes!("../../../../examples/browser/assets/metis-mark.ico");

    #[test]
    fn validates_project_mark() {
        validate(PROJECT_MARK).expect("generated project icon");
    }

    #[test]
    fn rejects_truncated_magic_and_dimension_mutations() {
        assert!(validate(&PROJECT_MARK[..5]).is_err());

        let mut invalid_type = PROJECT_MARK.to_vec();
        invalid_type[2] = 2;
        assert!(validate(&invalid_type).is_err());

        let mut invalid_dimension = PROJECT_MARK.to_vec();
        let payload_offset = usize::try_from(u32::from_le_bytes([
            invalid_dimension[18],
            invalid_dimension[19],
            invalid_dimension[20],
            invalid_dimension[21],
        ]))
        .expect("fixture offset fits host usize");
        invalid_dimension[6] = invalid_dimension[6].saturating_add(1);
        assert!(validate(&invalid_dimension).is_err());
        assert_eq!(
            &invalid_dimension[payload_offset..payload_offset + 8],
            PNG_SIGNATURE
        );

        let mut invalid_crc = PROJECT_MARK.to_vec();
        invalid_crc[payload_offset + 29] ^= 1;
        assert!(validate(&invalid_crc).is_err());
    }
}
