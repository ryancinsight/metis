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
    Ok(())
}
