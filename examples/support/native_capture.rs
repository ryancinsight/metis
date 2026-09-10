//! Artifact decoding used by the real native-host capture demonstration.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::io;
use std::path::PathBuf;

/// Parses the bounded command-line contract for the native capture.
pub(crate) fn parse_options(
    arguments: impl Iterator<Item = OsString>,
) -> Result<(PathBuf, String), io::Error> {
    let mut output = PathBuf::from("output/native-host");
    let mut source_revision = String::from("working-tree");
    let mut arguments = arguments;
    while let Some(argument) = arguments.next() {
        if argument == "--output" {
            let value = arguments
                .next()
                .ok_or_else(|| invalid_input("--output requires a directory"))?;
            output = PathBuf::from(value);
        } else if argument == "--source-revision" {
            let value = arguments
                .next()
                .ok_or_else(|| invalid_input("--source-revision requires a value"))?;
            source_revision = value
                .into_string()
                .map_err(|_| invalid_input("--source-revision must be UTF-8"))?;
        } else {
            return Err(invalid_input(
                "usage: native_host_capture [--output DIR] [--source-revision REV]",
            ));
        }
    }
    if source_revision.is_empty() || source_revision.chars().any(char::is_control) {
        return Err(invalid_input(
            "source revision must be non-empty and free of control characters",
        ));
    }
    Ok((output, source_revision))
}

/// Escapes one value for the capture's deterministic JSON document.
pub(crate) fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                write!(output, "\\u{:04x}", u32::from(character))
                    .expect("invariant: JSON string formatting cannot fail");
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

/// Decodes the bounded bitmap representation and returns row-major ARGB pixels.
pub(crate) fn decode_bmp(bytes: &[u8]) -> io::Result<(u32, u32, Vec<u32>)> {
    if bytes.len() < 54 || bytes.get(0..2) != Some(b"BM") {
        return Err(invalid_data(
            "bitmap header is truncated or has wrong magic",
        ));
    }
    let file_size = read_u32(bytes, 2)?;
    let offset = read_u32(bytes, 10)?;
    let dib_size = read_u32(bytes, 14)?;
    let width = read_u32(bytes, 18)?;
    let height = read_u32(bytes, 22)?;
    let planes = read_u16(bytes, 26)?;
    let bits = read_u16(bytes, 28)?;
    let compression = read_u32(bytes, 30)?;
    let image_size = read_u32(bytes, 34)?;
    if usize::try_from(file_size).map_err(|_| invalid_data("bitmap is too large"))? != bytes.len()
        || offset != 54
        || dib_size != 40
        || width == 0
        || height == 0
        || planes != 1
        || bits != 32
        || compression != 0
    {
        return Err(invalid_data(
            "bitmap header is outside the supported contract",
        ));
    }
    let pixel_count = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| invalid_data("bitmap pixel count overflow"))?;
    let expected_image_size = pixel_count
        .checked_mul(4)
        .ok_or_else(|| invalid_data("bitmap image size overflow"))?;
    if u64::from(image_size) != expected_image_size
        || 54_u64
            .checked_add(expected_image_size)
            .ok_or_else(|| invalid_data("bitmap file size overflow"))?
            != u64::try_from(bytes.len()).map_err(|_| invalid_data("bitmap is too large"))?
    {
        return Err(invalid_data("bitmap pixel payload length differs"));
    }
    let count = usize::try_from(pixel_count).map_err(|_| invalid_data("bitmap is too large"))?;
    let width = usize::try_from(width).map_err(|_| invalid_data("bitmap row is too large"))?;
    let height = usize::try_from(height).map_err(|_| invalid_data("bitmap is too large"))?;
    let stride = width
        .checked_mul(4)
        .ok_or_else(|| invalid_data("bitmap row size overflow"))?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(count)
        .map_err(|_| io::Error::other("bitmap decode reservation failed"))?;
    pixels.resize(count, 0);
    for source_row in 0..height {
        let source_start = 54_usize
            .checked_add(
                source_row
                    .checked_mul(stride)
                    .ok_or_else(|| invalid_data("bitmap row offset overflow"))?,
            )
            .ok_or_else(|| invalid_data("bitmap row offset overflow"))?;
        let target_row = height
            .checked_sub(source_row + 1)
            .ok_or_else(|| invalid_data("bitmap row order is invalid"))?;
        let target_start = target_row
            .checked_mul(width)
            .ok_or_else(|| invalid_data("bitmap target offset overflow"))?;
        for column in 0..width {
            let start = source_start
                .checked_add(
                    column
                        .checked_mul(4)
                        .ok_or_else(|| invalid_data("bitmap pixel offset overflow"))?,
                )
                .ok_or_else(|| invalid_data("bitmap pixel offset overflow"))?;
            let value = bytes
                .get(start..start + 4)
                .ok_or_else(|| invalid_data("bitmap pixel payload is truncated"))?;
            pixels[target_start + column] = u32::from_le_bytes(
                value
                    .try_into()
                    .map_err(|_| invalid_data("bitmap pixel width differs"))?,
            );
        }
    }
    Ok((
        u32::try_from(width).map_err(|_| invalid_data("bitmap width is too large"))?,
        u32::try_from(height).map_err(|_| invalid_data("bitmap height is too large"))?,
        pixels,
    ))
}

fn read_u16(bytes: &[u8], start: usize) -> io::Result<u16> {
    let end = start
        .checked_add(2)
        .ok_or_else(|| invalid_data("bitmap header offset overflow"))?;
    let value = bytes
        .get(start..end)
        .ok_or_else(|| invalid_data("bitmap header is truncated"))?;
    Ok(u16::from_le_bytes(value.try_into().map_err(|_| {
        invalid_data("bitmap header width differs")
    })?))
}

fn read_u32(bytes: &[u8], start: usize) -> io::Result<u32> {
    let end = start
        .checked_add(4)
        .ok_or_else(|| invalid_data("bitmap header offset overflow"))?;
    let value = bytes
        .get(start..end)
        .ok_or_else(|| invalid_data("bitmap header is truncated"))?;
    Ok(u32::from_le_bytes(value.try_into().map_err(|_| {
        invalid_data("bitmap header width differs")
    })?))
}

fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_input(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
