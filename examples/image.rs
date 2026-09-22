//! Renders a bounded raster image through the software display-list path.

use metis_platform::rasterizer::{CornerRadius, draw_rect_outline, fill_rect};
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
            rect: Rect::new(0, 0, 240, 240),
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
    let mut framebuffer = Framebuffer::new(240, 240)?;
    display.render_to(&mut framebuffer);
    // A square and a rounded panel side by side: the same fill and border
    // entry points, differing only in the corner radius they are given.
    let square_panel = Rect::new(20, 190, 92, 40);
    fill_rect(
        &mut framebuffer,
        square_panel,
        CornerRadius::SQUARE,
        Color::WHITE,
    );
    draw_rect_outline(
        &mut framebuffer,
        square_panel,
        2,
        CornerRadius::SQUARE,
        Color::BLUE,
    );
    let rounded_panel = Rect::new(128, 190, 92, 40);
    let radius = CornerRadius::clamped(14, rounded_panel);
    fill_rect(&mut framebuffer, rounded_panel, radius, Color::WHITE);
    draw_rect_outline(&mut framebuffer, rounded_panel, 2, radius, Color::BLUE);

    assert_example_pixels(&framebuffer, background);
    assert_panel_corners(&framebuffer, square_panel, rounded_panel, background);
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

/// Asserts the corner contract the two panels demonstrate.
fn assert_panel_corners(framebuffer: &Framebuffer, square: Rect, rounded: Rect, background: Color) {
    // A square panel paints its extreme corner; a rounded one leaves it clear.
    assert_eq!(framebuffer.get_pixel(square.x, square.y), Color::BLUE);
    assert_eq!(framebuffer.get_pixel(rounded.x, rounded.y), background);
    // Both keep their straight edges and their interiors.
    let midpoint = |rect: Rect| (rect.x + rect.width / 2, rect.y);
    for rect in [square, rounded] {
        let (x, y) = midpoint(rect);
        assert_eq!(framebuffer.get_pixel(x, y), Color::BLUE);
        assert_eq!(
            framebuffer.get_pixel(x, y + rect.height / 2),
            Color::WHITE,
            "panel interior is not filled"
        );
    }
    // The rounded corner is antialiased, so its arc carries partial coverage.
    let partial = (rounded.x..rounded.x + 16)
        .flat_map(|x| (rounded.y..rounded.y + 16).map(move |y| (x, y)))
        .filter(|(x, y)| {
            let pixel = framebuffer.get_pixel(*x, *y);
            pixel != background && pixel != Color::WHITE && pixel != Color::BLUE
        })
        .count();
    assert!(partial >= 8, "rounded corner is not antialiased: {partial}");
}
