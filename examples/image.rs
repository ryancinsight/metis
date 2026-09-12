//! Renders a bounded raster image through the software display-list path.

use metis_platform::{Color, Framebuffer, Rect};
use metis_ui_lang::{
    DisplayCommand, DisplayList, ImagePlacement, ImageSampling, ImageTransform, RasterImage,
};
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
        image.clone(),
        Rect::new(0, 0, 3, 2),
        Rect::new(24, 48, 120, 80),
        ImageSampling::Nearest,
    )?;
    let rotated = ImagePlacement::new(
        image,
        Rect::new(0, 0, 3, 2),
        Rect::new(156, 30, 72, 108),
        ImageSampling::Nearest,
    )?
    .with_transform(ImageTransform::RotateClockwise);
    let mut display = DisplayList {
        commands: vec![DisplayCommand::FillRect {
            rect: Rect::new(0, 0, 240, 180),
            color: background,
        }],
    };
    display.append_image(placement)?;
    display.append_image(rotated)?;
    display.append_line((20, 20), (148, 20), Color::LIGHT_GRAY)?;
    display.append_line((152, 20), (232, 20), Color::LIGHT_GRAY)?;
    display.append_line((20, 144), (148, 144), Color::LIGHT_GRAY)?;
    display.append_line((152, 144), (232, 144), Color::LIGHT_GRAY)?;
    let mut framebuffer = Framebuffer::new(240, 180)?;
    display.render_to(&mut framebuffer);

    for (x, y, expected) in [
        (24, 48, Color::RED),
        (64, 48, Color::GREEN),
        (104, 48, Color::BLUE),
        (24, 88, Color::rgb(255, 200, 0)),
        (64, 88, Color::rgb(0, 200, 220)),
        (104, 88, Color::rgb(200, 0, 220)),
        (174, 48, Color::rgb(255, 200, 0)),
        (210, 48, Color::RED),
        (174, 84, Color::rgb(0, 200, 220)),
        (210, 84, Color::GREEN),
        (174, 120, Color::rgb(200, 0, 220)),
        (210, 120, Color::BLUE),
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
