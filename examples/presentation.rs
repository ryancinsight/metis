//! Renders the declared form through Iris into a BMP for visual inspection.
use iris::render::RenderBackend;
use metis_frontend::CLINICAL_SCREEN_XML;
use metis_platform::Framebuffer;
use metis_ui_lang::{layout::compute_layout, parser::parse_markup};
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let document = parse_markup(CLINICAL_SCREEN_XML)?;
    let width = 800_u32;
    let height = 600_u32;
    let display = compute_layout(&document, i32::try_from(width)?, i32::try_from(height)?)?;
    let mut framebuffer = Framebuffer::new(width, height)?;
    let pixels = framebuffer.render(&display)?;
    std::fs::create_dir_all("output")?;
    let mut bitmap = std::fs::File::create("output/form.bmp")?;
    // BITMAPFILEHEADER followed by 40-byte BITMAPINFOHEADER, 32-bit BI_RGB.
    let image_size = width * height * 4;
    bitmap.write_all(b"BM")?;
    bitmap.write_all(&(54 + image_size).to_le_bytes())?;
    bitmap.write_all(&[0; 4])?;
    bitmap.write_all(&54_u32.to_le_bytes())?;
    bitmap.write_all(&40_u32.to_le_bytes())?;
    bitmap.write_all(&width.to_le_bytes())?;
    bitmap.write_all(&height.to_le_bytes())?;
    bitmap.write_all(&1_u16.to_le_bytes())?;
    bitmap.write_all(&32_u16.to_le_bytes())?;
    bitmap.write_all(&0_u32.to_le_bytes())?;
    bitmap.write_all(&image_size.to_le_bytes())?;
    bitmap.write_all(&[0; 16])?;
    for row in pixels.chunks_exact(usize::try_from(width)?).rev() {
        for pixel in row {
            bitmap.write_all(&pixel.to_le_bytes())?;
        }
    }
    bitmap.flush()?;
    let mut snapshot = std::io::BufWriter::new(std::fs::File::create("output/form.svg")?);
    writeln!(
        snapshot,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" shape-rendering=\"crispEdges\">"
    )?;
    writeln!(snapshot, "<title>Metis form software framebuffer</title>")?;
    // Encode the actual raster as horizontal runs; no text or layout is rebuilt.
    for (y, row) in pixels.chunks_exact(usize::try_from(width)?).enumerate() {
        let mut x = 0;
        while let Some(&pixel) = row.get(x) {
            let length = row[x..].iter().take_while(|&&next| next == pixel).count();
            let [alpha, red, green, blue] = pixel.to_be_bytes();
            writeln!(
                snapshot,
                "<path fill=\"#{red:02x}{green:02x}{blue:02x}\" fill-opacity=\"{}\" d=\"M{x} {y}h{length}v1H{x}z\"/>",
                f64::from(alpha) / 255.0
            )?;
            x += length;
        }
    }
    writeln!(snapshot, "</svg>")?;
    snapshot.flush()?;
    Ok(())
}
