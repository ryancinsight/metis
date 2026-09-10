//! Bounded framebuffer artifact encoding shared by the runnable examples.

use metis_platform::Framebuffer;
use std::fmt::Write as _;
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

/// Encodes a framebuffer as complete ordered SVG raster runs.
pub(crate) fn svg_text(framebuffer: &Framebuffer) -> io::Result<String> {
    let width = framebuffer.width();
    let height = framebuffer.height();
    let mut output = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" shape-rendering=\"crispEdges\">\n<title>Metis form software framebuffer</title>\n"
    );
    for y in 0..height {
        let y = i32::try_from(y).map_err(|_| invalid_data("SVG row exceeds coordinates"))?;
        let mut x = 0_u32;
        while x < width {
            let x_i32 =
                i32::try_from(x).map_err(|_| invalid_data("SVG column exceeds coordinates"))?;
            let color = framebuffer.get_pixel(x_i32, y);
            let mut length = 1_u32;
            while x + length < width
                && framebuffer.get_pixel(
                    i32::try_from(x + length)
                        .map_err(|_| invalid_data("SVG column exceeds coordinates"))?,
                    y,
                ) == color
            {
                length += 1;
            }
            let opacity = f64::from(color.a) / 255.0;
            writeln!(
                output,
                "<path fill=\"#{:02x}{:02x}{:02x}\" fill-opacity=\"{opacity}\" d=\"M{x} {y}h{length}v1H{x}z\"/>",
                color.r, color.g, color.b
            )
            .map_err(|_| io::Error::other("SVG string formatting failed"))?;
            x += length;
        }
    }
    output.push_str("</svg>\n");
    Ok(output)
}

fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
