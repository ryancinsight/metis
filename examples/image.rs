//! Renders a bounded raster image through the software display-list path.

use metis_platform::{Color, Framebuffer, Rect};
use metis_ui_lang::{DisplayCommand, DisplayList, ImagePlacement, ImageSampling, RasterImage};
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all("output")?;
    let background = Color::rgb(15, 23, 42);
    let image = RasterImage::new(
        3,
        2,
        vec![
            Color::RED,
            Color::GREEN,
            Color::BLUE,
            Color::rgb(255, 200, 0),
            Color::rgb(0, 200, 220),
            Color::rgb(200, 0, 220),
        ],
    )?;
    let placement = ImagePlacement::new(
        image,
        Rect::new(0, 0, 3, 2),
        Rect::new(30, 30, 180, 120),
        ImageSampling::Nearest,
    )?;
    let mut display = DisplayList {
        commands: vec![DisplayCommand::FillRect {
            rect: Rect::new(0, 0, 240, 180),
            color: background,
        }],
    };
    display.append_image(placement)?;
    let mut framebuffer = Framebuffer::new(240, 180)?;
    display.render_to(&mut framebuffer);

    for (x, y, expected) in [
        (30, 30, Color::RED),
        (90, 30, Color::GREEN),
        (150, 30, Color::BLUE),
        (30, 90, Color::rgb(255, 200, 0)),
        (90, 90, Color::rgb(0, 200, 220)),
        (150, 90, Color::rgb(200, 0, 220)),
    ] {
        assert_eq!(framebuffer.get_pixel(x, y), expected);
    }
    assert_eq!(framebuffer.get_pixel(0, 0), background);
    write_bmp(&framebuffer, "output/image-placement.bmp")?;
    write_svg(&framebuffer, "output/image-placement.svg")?;
    Ok(())
}

fn write_bmp(framebuffer: &Framebuffer, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let width = framebuffer.width();
    let height = framebuffer.height();
    let image_size = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or("BMP image size overflow")?;
    let file = std::fs::File::create(path)?;
    let mut output = std::io::BufWriter::new(file);
    output.write_all(b"BM")?;
    output.write_all(&(54_u32 + image_size).to_le_bytes())?;
    output.write_all(&[0; 4])?;
    output.write_all(&54_u32.to_le_bytes())?;
    output.write_all(&40_u32.to_le_bytes())?;
    output.write_all(&width.to_le_bytes())?;
    output.write_all(&height.to_le_bytes())?;
    output.write_all(&1_u16.to_le_bytes())?;
    output.write_all(&32_u16.to_le_bytes())?;
    output.write_all(&0_u32.to_le_bytes())?;
    output.write_all(&image_size.to_le_bytes())?;
    output.write_all(&[0; 16])?;
    for row in framebuffer
        .pixels()
        .chunks_exact(usize::try_from(width)?)
        .rev()
    {
        for pixel in row {
            output.write_all(&pixel.to_le_bytes())?;
        }
    }
    output.flush()?;
    Ok(())
}

fn write_svg(framebuffer: &Framebuffer, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let width = framebuffer.width();
    let height = framebuffer.height();
    let file = std::fs::File::create(path)?;
    let mut output = std::io::BufWriter::new(file);
    writeln!(
        output,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" shape-rendering=\"crispEdges\">"
    )?;
    writeln!(output, "<title>Metis form software framebuffer</title>")?;
    for y in 0..height {
        let y = i32::try_from(y)?;
        let mut x = 0_u32;
        while x < width {
            let x_i32 = i32::try_from(x)?;
            let color = framebuffer.get_pixel(x_i32, y);
            let mut length = 1_u32;
            while x + length < width
                && framebuffer.get_pixel(i32::try_from(x + length)?, y) == color
            {
                length += 1;
            }
            writeln!(
                output,
                "<path fill=\"#{:02x}{:02x}{:02x}\" fill-opacity=\"{}\" d=\"M{x} {y}h{length}v1H{x}z\"/>",
                color.r,
                color.g,
                color.b,
                f32::from(color.a) / 255.0
            )?;
            x += length;
        }
    }
    writeln!(output, "</svg>")?;
    output.flush()?;
    Ok(())
}
