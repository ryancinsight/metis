//! Renders a bounded raster image through the software display-list path.

use metis_platform::{Color, Framebuffer, Rect};
use metis_ui_lang::{DisplayCommand, DisplayList, ImagePlacement, ImageSampling, RasterImage};
#[path = "support/framebuffer.rs"]
mod framebuffer_artifacts;

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
    std::fs::write(
        "output/image-placement.bmp",
        framebuffer_artifacts::bmp_bytes(&framebuffer)?,
    )?;
    std::fs::write(
        "output/image-placement.svg",
        framebuffer_artifacts::svg_text(&framebuffer)?,
    )?;
    Ok(())
}
