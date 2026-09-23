//! Presents a decoded, aspect-preserving orientation fixture on the Windows host.

#[path = "support/framebuffer.rs"]
mod framebuffer_artifacts;
#[path = "support/jpeg_arithmetic_gallery.rs"]
mod jpeg_arithmetic_gallery;
#[path = "support/jpeg_gallery.rs"]
mod jpeg_gallery;

use metis_platform::rasterizer::CornerRadius;
use metis_platform::typeface::{TextSize, TextStyle, draw_text};
use metis_platform::{Color, Framebuffer, Rect, draw_rect_outline, fill_rect};
use metis_ui_lang::asset::AssetErrorKind;
use metis_ui_lang::{ImagePlacement, ImageTransform, RasterImage};
use std::error::Error;
use std::io;

const FRAME_WIDTH: u32 = 800;
const FRAME_HEIGHT: u32 = 580;
const PANEL_WIDTH: i32 = 140;
const PANEL_HEIGHT: i32 = 156;
const PANEL_Y: i32 = 76;
const PANEL_BACKGROUND: Color = Color::rgb(30, 41, 59);
const PAGE_BACKGROUND: Color = Color::rgb(8, 15, 29);
const OUTLINE: Color = Color::rgb(100, 116, 139);
const TEXT: Color = Color::rgb(241, 245, 249);
const TRANSLUCENT_BLEND: Color = Color::rgb(125, 48, 135);
const FIXTURE_PNG: &[u8] = include_bytes!("assets/orientation.png");

const FIXTURE_PIXELS: [Color; 6] = [
    Color::rgba(240, 50, 60, 255),
    Color::rgba(60, 200, 90, 255),
    Color::rgba(40, 120, 235, 255),
    Color::rgba(255, 190, 35, 255),
    Color::rgba(35, 205, 220, 255),
    Color::rgba(220, 55, 210, 128),
];

const CASES: [(&str, ImageTransform, [Color; 4]); 5] = [
    (
        "IDENTITY",
        ImageTransform::Identity,
        [
            FIXTURE_PIXELS[0],
            FIXTURE_PIXELS[1],
            FIXTURE_PIXELS[4],
            TRANSLUCENT_BLEND,
        ],
    ),
    (
        "FLIP H",
        ImageTransform::FlipHorizontal,
        [
            FIXTURE_PIXELS[1],
            FIXTURE_PIXELS[0],
            TRANSLUCENT_BLEND,
            FIXTURE_PIXELS[4],
        ],
    ),
    (
        "FLIP V",
        ImageTransform::FlipVertical,
        [
            FIXTURE_PIXELS[4],
            TRANSLUCENT_BLEND,
            FIXTURE_PIXELS[0],
            FIXTURE_PIXELS[1],
        ],
    ),
    (
        "ROTATE CW",
        ImageTransform::RotateClockwise,
        [
            FIXTURE_PIXELS[4],
            FIXTURE_PIXELS[0],
            TRANSLUCENT_BLEND,
            FIXTURE_PIXELS[1],
        ],
    ),
    (
        "ROTATE CCW",
        ImageTransform::RotateCounterClockwise,
        [
            FIXTURE_PIXELS[1],
            TRANSLUCENT_BLEND,
            FIXTURE_PIXELS[0],
            FIXTURE_PIXELS[4],
        ],
    ),
];

fn render_png_gallery(frame: &mut Framebuffer) -> Result<(), Box<dyn Error>> {
    let image = RasterImage::decode(FIXTURE_PNG)?;
    assert_eq!((image.width(), image.height()), (2, 3));
    assert_eq!(image.pixels(), FIXTURE_PIXELS);
    let truncated_length = FIXTURE_PNG
        .len()
        .checked_sub(8)
        .ok_or_else(|| io::Error::other("orientation fixture is shorter than IEND"))?;
    let truncated = FIXTURE_PNG
        .get(..truncated_length)
        .ok_or_else(|| io::Error::other("truncated fixture range is outside its bytes"))?;
    let Err(rejection) = RasterImage::decode(truncated) else {
        return Err(io::Error::other("truncated PNG was admitted").into());
    };
    assert_eq!(rejection.kind(), AssetErrorKind::Malformed);

    draw_text(
        frame,
        18,
        16,
        "NATIVE PNG: ASPECT ALPHA ORIENTATION",
        TextStyle::new(
            TEXT,
            TextSize::new(24.0).expect("invariant: 24 px is a valid text size"),
        ),
    );

    for (index, (label, transform, expected_corners)) in CASES.into_iter().enumerate() {
        let index = i32::try_from(index)?;
        let bounds = Rect::new(18 + index * 156, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT);
        draw_text(
            frame,
            bounds.x,
            52,
            label,
            TextStyle::new(
                TEXT,
                TextSize::new(14.0).expect("invariant: 14 px is a valid text size"),
            ),
        );
        draw_rect_outline(
            frame,
            Rect::new(
                bounds.x - 1,
                bounds.y - 1,
                bounds.width + 2,
                bounds.height + 2,
            ),
            1,
            CornerRadius::SQUARE,
            OUTLINE,
        );
        fill_rect(frame, bounds, CornerRadius::SQUARE, PANEL_BACKGROUND);

        let placement = ImagePlacement::contain(image.clone(), bounds, transform)?;
        let expected_destination = match transform {
            ImageTransform::RotateClockwise | ImageTransform::RotateCounterClockwise => {
                Rect::new(bounds.x, bounds.y + 31, 140, 93)
            }
            _ => Rect::new(bounds.x + 18, bounds.y, 104, 156),
        };
        assert_eq!(placement.destination(), expected_destination);
        assert_eq!(placement.transform(), transform);
        placement.render_to(frame);

        let destination = placement.destination();
        let samples = [
            (destination.x, destination.y),
            (destination.x + destination.width - 1, destination.y),
            (destination.x, destination.y + destination.height - 1),
            (
                destination.x + destination.width - 1,
                destination.y + destination.height - 1,
            ),
        ];
        for ((x, y), expected) in samples.into_iter().zip(expected_corners) {
            assert_eq!(frame.get_pixel(x, y), expected);
        }
        let letterbox = if destination.x > bounds.x {
            (bounds.x, bounds.y + bounds.height / 2)
        } else {
            (bounds.x + bounds.width / 2, bounds.y)
        };
        assert_eq!(frame.get_pixel(letterbox.0, letterbox.1), PANEL_BACKGROUND);
    }
    Ok(())
}

fn render_jpeg_gallery(frame: &mut Framebuffer) -> Result<(), Box<dyn Error>> {
    jpeg_gallery::render(frame)?;
    jpeg_arithmetic_gallery::render(frame)
}

fn render_frame() -> Result<Framebuffer, Box<dyn Error>> {
    let mut frame = Framebuffer::new(FRAME_WIDTH, FRAME_HEIGHT)?;
    frame.clear(PAGE_BACKGROUND);
    render_png_gallery(&mut frame)?;
    render_jpeg_gallery(&mut frame)?;
    draw_text(
        &mut frame,
        18,
        558,
        "TRUNCATED PNG + JPEG: REJECTED (MALFORMED)",
        TextStyle::new(
            Color::rgb(74, 222, 128),
            TextSize::new(14.0).expect("invariant: 14 px is a valid text size"),
        ),
    );

    Ok(frame)
}

fn write_expected_frame(frame: &Framebuffer) -> io::Result<()> {
    let output = std::path::Path::new("output/native-image");
    std::fs::create_dir_all(output)?;
    std::fs::write(
        output.join("expected-frame.bmp"),
        framebuffer_artifacts::bmp_bytes(frame)?,
    )?;
    std::fs::write(
        output.join("expected-frame.png"),
        framebuffer_artifacts::png_bytes(frame)?,
    )?;
    Ok(())
}

#[cfg(windows)]
fn run_native(frame: Framebuffer, visible: bool) -> Result<(), Box<dyn Error>> {
    use metis_platform::native::{
        NativeApplication, NativeFlow, WindowConfig, WindowEvent, WindowVisibility,
        run_native_application,
    };
    use std::convert::Infallible;
    use std::time::{Duration, Instant};

    const HOST_DEADLINE: Duration = Duration::from_secs(20);
    const EVENT_WAIT: Duration = Duration::from_millis(100);

    struct ImageApplication {
        frame: Framebuffer,
        visible: bool,
        deadline: Instant,
    }

    impl NativeApplication for ImageApplication {
        type Error = Infallible;

        fn framebuffer(&self) -> &Framebuffer {
            &self.frame
        }

        fn handle_events(&mut self, events: &[WindowEvent]) -> Result<NativeFlow, Self::Error> {
            if events
                .iter()
                .any(|event| matches!(event, WindowEvent::CloseRequested | WindowEvent::Destroyed))
                || !self.visible
                || Instant::now() >= self.deadline
            {
                return Ok(NativeFlow::Exit);
            }
            Ok(NativeFlow::Continue { repaint: false })
        }
    }

    let visibility = if visible {
        WindowVisibility::Visible
    } else {
        WindowVisibility::Hidden
    };
    let config = WindowConfig::with_visibility(
        "Metis native image V06",
        FRAME_WIDTH,
        FRAME_HEIGHT,
        visibility,
    )?;
    let application = ImageApplication {
        frame,
        visible,
        deadline: Instant::now() + HOST_DEADLINE,
    };
    run_native_application(&config, application, EVENT_WAIT)?;
    Ok(())
}

fn visible_mode() -> io::Result<bool> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    match arguments.as_slice() {
        [] => Ok(false),
        [argument] if argument == "--visible" => Ok(true),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: native_image [--visible]",
        )),
    }
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn Error>> {
    let visible = visible_mode()?;
    let frame = render_frame()?;
    write_expected_frame(&frame)?;
    run_native(frame, visible)
}

#[cfg(not(windows))]
fn main() -> Result<(), Box<dyn Error>> {
    if visible_mode()? {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "visible native image presentation requires Windows",
        )
        .into());
    }
    let frame = render_frame()?;
    write_expected_frame(&frame)?;
    Ok(())
}
