//! Renders a bounded raster image through the software display-list path.

use metis_platform::rasterizer::CornerRadius;
use metis_platform::{Color, Framebuffer, Rect};
use metis_ui_lang::{
    AffineTransform, DisplayCommand, DisplayList, ImagePlacement, ImageSampling, ImageTransform,
    LineCap, LineJoin, RasterImage, StrokeWidth,
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
        image.clone(),
        Rect::new(0, 0, 3, 2),
        Rect::new(156, 30, 72, 108),
        ImageSampling::Nearest,
    )?
    .with_transform(ImageTransform::RotateClockwise);
    let affine = ImagePlacement::new(
        image,
        Rect::new(0, 0, 3, 2),
        Rect::new(24, 148, 72, 24),
        ImageSampling::Nearest,
    )?
    .with_transform(ImageTransform::Affine(AffineTransform::new(
        1.0, 0.0, 0.25, 1.0, 0.0, 0.0,
    )?));
    let mut display = DisplayList {
        commands: vec![DisplayCommand::FillRect {
            rect: Rect::new(0, 0, 240, 180),
            radius: CornerRadius::SQUARE,
            color: background,
        }],
    };
    display.append_image(placement)?;
    display.append_image(rotated)?;
    display.append_image(affine)?;
    let stroke = StrokeWidth::new(3)?;
    display.append_polyline(
        &[(20, 20), (148, 20), (148, 144), (20, 144), (20, 20)],
        stroke,
        LineCap::Round,
        LineJoin::Round,
        Color::LIGHT_GRAY,
    )?;
    display.append_polyline(
        &[(152, 20), (232, 20), (232, 144), (152, 144), (152, 20)],
        stroke,
        LineCap::Square,
        LineJoin::Bevel,
        Color::LIGHT_GRAY,
    )?;
    display.append_polyline(
        &[(96, 165), (108, 150), (120, 165)],
        stroke,
        LineCap::Butt,
        LineJoin::Miter,
        Color::GRAY,
    )?;
    let mut framebuffer = Framebuffer::new(240, 180)?;
    display.render_to(&mut framebuffer);

    assert_example_pixels(&framebuffer, background);
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

fn assert_example_pixels(framebuffer: &Framebuffer, background: Color) {
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
        (36, 152, Color::RED),
        (60, 152, Color::GREEN),
        (84, 152, Color::BLUE),
        (36, 164, Color::rgb(255, 200, 0)),
        (60, 164, Color::rgb(0, 200, 220)),
        (84, 164, Color::rgb(200, 0, 220)),
    ] {
        assert_eq!(framebuffer.get_pixel(x, y), expected);
    }
    assert_eq!(framebuffer.get_pixel(0, 0), background);
}
