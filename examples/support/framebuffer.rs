//! Bounded framebuffer artifact encoding shared by the runnable examples.

use consus_compression::codec::deflate::DeflateCodec;
use consus_compression::{Checksum, Codec, CompressionLevel, Crc32};
use metis_platform::Framebuffer;
use std::io;

/// Encodes a framebuffer as a bounded 32-bit bottom-up bitmap.
pub(crate) fn bmp_bytes(framebuffer: &Framebuffer) -> io::Result<Vec<u8>> {
    let width = framebuffer.width();
    let height = framebuffer.height();
    let pixel_count = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| invalid_data("bitmap pixel count overflow"))?;
    let image_size = pixel_count
        .checked_mul(4)
        .ok_or_else(|| invalid_data("bitmap image size overflow"))?;
    let file_size = 54_u64
        .checked_add(image_size)
        .ok_or_else(|| invalid_data("bitmap file size overflow"))?;
    let file_size = usize::try_from(file_size).map_err(|_| invalid_data("bitmap is too large"))?;
    let image_size = u32::try_from(image_size).map_err(|_| invalid_data("bitmap is too large"))?;
    let file_size_u32 =
        u32::try_from(file_size).map_err(|_| invalid_data("bitmap is too large"))?;
    let row_width = usize::try_from(width).map_err(|_| invalid_data("bitmap row is too large"))?;

    let mut output = Vec::new();
    output
        .try_reserve_exact(file_size)
        .map_err(|_| io::Error::other("bitmap output reservation failed"))?;
    output.extend_from_slice(b"BM");
    output.extend_from_slice(&file_size_u32.to_le_bytes());
    output.extend_from_slice(&[0; 4]);
    output.extend_from_slice(&54_u32.to_le_bytes());
    output.extend_from_slice(&40_u32.to_le_bytes());
    output.extend_from_slice(&width.to_le_bytes());
    output.extend_from_slice(&height.to_le_bytes());
    output.extend_from_slice(&1_u16.to_le_bytes());
    output.extend_from_slice(&32_u16.to_le_bytes());
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&image_size.to_le_bytes());
    output.extend_from_slice(&[0; 16]);

    let mut rows = framebuffer.pixels().chunks_exact(row_width);
    for row in rows.by_ref().rev() {
        for pixel in row {
            output.extend_from_slice(&pixel.to_le_bytes());
        }
    }
    if !rows.remainder().is_empty() {
        return Err(invalid_data("bitmap pixels do not form complete rows"));
    }
    Ok(output)
}

/// PNG file signature (ISO/IEC 15948, section 5.2).
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
/// Bit depth, color type (6, RGBA), compression, filter and interlace methods
/// of the header chunk: 8-bit straight RGBA, deflate, standard filters, no
/// interlace. `scripts/visual.py` admits exactly this header.
const PNG_LAYOUT: [u8; 5] = [8, 6, 0, 0, 0];
/// DEFLATE level: the smallest output, for committed evidence.
const PNG_DEFLATE_LEVEL: CompressionLevel = CompressionLevel(9);

/// Encodes a framebuffer as an 8-bit straight-alpha RGBA PNG.
///
/// Every row is written unfiltered (filter type 0): the renderer's flat fills
/// and ramps compress best that way under DEFLATE (about 50 KB per 800 x 600
/// form capture, against about 64 KB with adaptive filters), and the review
/// decoder in `scripts/visual.py` admits exactly this encoding. The zlib
/// stream and chunk CRCs come from Consus.
pub(crate) fn png_bytes(framebuffer: &Framebuffer) -> io::Result<Vec<u8>> {
    let row_width =
        usize::try_from(framebuffer.width()).map_err(|_| invalid_data("PNG row is too large"))?;
    let mut scanlines = Vec::new();
    scanlines
        .try_reserve_exact(
            framebuffer.pixels().len() * 4 + framebuffer.pixels().len() / row_width.max(1),
        )
        .map_err(|_| io::Error::other("PNG scanline reservation failed"))?;
    let mut rows = framebuffer.pixels().chunks_exact(row_width);
    for row in rows.by_ref() {
        scanlines.push(0);
        for pixel in row {
            let [alpha, red, green, blue] = pixel.to_be_bytes();
            scanlines.extend_from_slice(&[red, green, blue, alpha]);
        }
    }
    if !rows.remainder().is_empty() {
        return Err(invalid_data("PNG pixels do not form complete rows"));
    }
    let compressed = DeflateCodec
        .compress(&scanlines, PNG_DEFLATE_LEVEL)
        .map_err(|error| io::Error::other(error.to_string()))?;
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&framebuffer.width().to_be_bytes());
    header.extend_from_slice(&framebuffer.height().to_be_bytes());
    header.extend_from_slice(&PNG_LAYOUT);
    let mut output = Vec::new();
    output
        .try_reserve_exact(PNG_SIGNATURE.len() + 3 * 12 + header.len() + compressed.len())
        .map_err(|_| io::Error::other("PNG output reservation failed"))?;
    output.extend_from_slice(&PNG_SIGNATURE);
    for (kind, data) in [
        (b"IHDR", header.as_slice()),
        (b"IDAT", &compressed),
        (b"IEND", &[]),
    ] {
        png_chunk(&mut output, *kind, data)?;
    }
    Ok(output)
}

/// Appends one PNG chunk: length, type, data and the CRC-32 of type and data.
fn png_chunk(output: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) -> io::Result<()> {
    let length = u32::try_from(data.len()).map_err(|_| invalid_data("PNG chunk is too large"))?;
    output.extend_from_slice(&length.to_be_bytes());
    let mut crc = Crc32::new();
    crc.update(&kind);
    crc.update(data);
    output.extend_from_slice(&kind);
    output.extend_from_slice(data);
    output.extend_from_slice(&crc.finalize().to_be_bytes());
    Ok(())
}

fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
