use metis_platform::rasterizer::CornerRadius;
use metis_platform::typeface::{TextSize, TextStyle, draw_text};
use metis_platform::{Color, Framebuffer, Rect, draw_rect_outline, fill_rect};
use metis_ui_lang::asset::AssetErrorKind;
use metis_ui_lang::{ImagePlacement, ImageTransform, RasterImage};
use std::{error::Error, io};

use super::{OUTLINE, PANEL_BACKGROUND, TEXT};
#[path = "../../crates/metis-ui-lang/src/asset/tests/jpeg_fixtures.rs"]
mod jpeg_fixtures;

const PANEL_WIDTH: i32 = 174;
const PANEL_HEIGHT: i32 = 84;
const LABELS: [&str; 8] = [
    "1 NORMAL",
    "2 MIRROR H",
    "3 ROTATE 180",
    "4 MIRROR V",
    "5 TRANSPOSE",
    "6 ROTATE CW",
    "7 TRANSVERSE",
    "8 ROTATE CCW",
];
const BLOCK_ORDERS: [[usize; 6]; 8] = [
    [0, 1, 2, 3, 4, 5],
    [2, 1, 0, 5, 4, 3],
    [5, 4, 3, 2, 1, 0],
    [3, 4, 5, 0, 1, 2],
    [0, 3, 1, 4, 2, 5],
    [3, 0, 4, 1, 5, 2],
    [5, 2, 4, 1, 3, 0],
    [2, 5, 1, 4, 0, 3],
];
const CORNER_ORDERS: [[usize; 4]; 8] = [
    [0, 1, 2, 3],
    [1, 0, 3, 2],
    [3, 2, 1, 0],
    [2, 3, 0, 1],
    [0, 2, 1, 3],
    [2, 0, 3, 1],
    [3, 1, 2, 0],
    [1, 3, 0, 2],
];
// Authored as a 3-by-2 color grid and encoded with Pillow 12.1.1 at quality 92,
// 4:4:4 sampling, to keep the JPEG evidence deterministic and independently produced.
const FIXTURE: &[u8] = include_bytes!("../assets/orientation.jpg");

fn jpeg_with_orientation(orientation: u8) -> io::Result<Vec<u8>> {
    if !(1..=8).contains(&orientation) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "EXIF orientation must be in 1..=8",
        ));
    }
    if FIXTURE.get(..2) != Some(&[0xff, 0xd8]) {
        return Err(io::Error::other("JPEG fixture has no SOI marker"));
    }

    // A little-endian TIFF IFD containing only tag 0x0112 (orientation).
    let mut app1 = Vec::with_capacity(36);
    app1.extend_from_slice(&[0xff, 0xe1, 0x00, 0x22]);
    app1.extend_from_slice(b"Exif\0\0");
    app1.extend_from_slice(&[0x49, 0x49, 0x2a, 0x00, 0x08, 0x00, 0x00, 0x00]);
    app1.extend_from_slice(&[0x01, 0x00]);
    app1.extend_from_slice(&[
        0x12,
        0x01,
        0x03,
        0x00,
        0x01,
        0x00,
        0x00,
        0x00,
        orientation,
        0x00,
        0x00,
        0x00,
    ]);
    app1.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    assert_eq!(app1.len(), 36);

    let mut fixture = Vec::with_capacity(FIXTURE.len() + app1.len());
    fixture.extend_from_slice(&FIXTURE[..2]);
    fixture.extend_from_slice(&app1);
    fixture.extend_from_slice(&FIXTURE[2..]);
    Ok(fixture)
}

fn pixel_at(image: &RasterImage, x: u32, y: u32) -> io::Result<Color> {
    let index = y
        .checked_mul(image.width())
        .and_then(|row| row.checked_add(x))
        .ok_or_else(|| io::Error::other("JPEG sample index overflowed"))?;
    image
        .pixels()
        .get(
            usize::try_from(index)
                .map_err(|_| io::Error::other("JPEG sample index exceeds host usize"))?,
        )
        .copied()
        .ok_or_else(|| io::Error::other("JPEG sample was outside the raster"))
}

fn block_samples(image: &RasterImage, columns: u32, rows: u32) -> io::Result<[Color; 6]> {
    if image.width() != columns * 16 || image.height() != rows * 16 {
        return Err(io::Error::other(
            "JPEG block grid has unexpected dimensions",
        ));
    }
    let mut samples = Vec::with_capacity(6);
    for row in 0..rows {
        for column in 0..columns {
            samples.push(pixel_at(image, column * 16 + 8, row * 16 + 8)?);
        }
    }
    samples
        .try_into()
        .map_err(|_| io::Error::other("JPEG block grid does not contain six samples"))
}

fn corner_samples(image: &RasterImage) -> io::Result<[Color; 4]> {
    Ok([
        pixel_at(image, 0, 0)?,
        pixel_at(image, image.width() - 1, 0)?,
        pixel_at(image, 0, image.height() - 1)?,
        pixel_at(image, image.width() - 1, image.height() - 1)?,
    ])
}

pub(super) fn render(frame: &mut Framebuffer) -> Result<(), Box<dyn Error>> {
    render_precision_samples(frame)?;
    let base = RasterImage::decode(FIXTURE)?;
    assert_eq!((base.width(), base.height()), (48, 32));
    let base_blocks = block_samples(&base, 3, 2)?;
    let base_corners = corner_samples(&base)?;
    // Pillow's independent decode samples the constant interiors of the six
    // blocks. Against BT.601, the largest 16.8 coefficient error is blue's
    // |454/256 - 1.772| * 128 = 0.184 sample; the combined green error is
    // smaller. Rounding either reconstruction therefore differs by at most one.
    let reference = [
        [230_u8, 44, 55],
        [45, 185, 80],
        [40, 105, 225],
        [240, 175, 31],
        [29, 190, 210],
        [204, 45, 191],
    ];
    for (actual, expected) in base_blocks.iter().zip(reference) {
        for (channel, reference) in [actual.r, actual.g, actual.b].into_iter().zip(expected) {
            assert!(channel.abs_diff(reference) <= 1);
        }
        assert_eq!(actual.a, 255);
    }
    draw_text(
        frame,
        18,
        302,
        "NATIVE JPEG/EXIF: ALL 8 ORIENTATIONS NORMALIZED",
        TextStyle::new(
            TEXT,
            TextSize::new(24.0).expect("invariant: 24 px is a valid text size"),
        ),
    );

    for (index, label) in LABELS.into_iter().enumerate() {
        render_orientation(frame, &base_blocks, &base_corners, index, label)?;
    }

    let truncated_length = FIXTURE
        .len()
        .checked_sub(2)
        .ok_or_else(|| io::Error::other("JPEG fixture is shorter than EOI"))?;
    let truncated = FIXTURE
        .get(..truncated_length)
        .ok_or_else(|| io::Error::other("truncated JPEG range is outside its bytes"))?;
    let Err(rejection) = RasterImage::decode(truncated) else {
        return Err(io::Error::other("truncated JPEG was admitted").into());
    };
    assert_eq!(rejection.kind(), AssetErrorKind::Malformed);

    Ok(())
}

fn render_precision_samples(frame: &mut Framebuffer) -> Result<(), Box<dyn Error>> {
    let gray = RasterImage::decode(&jpeg_fixtures::lossless_gray(12))?;
    let rgb = RasterImage::decode(&jpeg_fixtures::direct_rgb_twelve())?;
    assert_eq!(gray.pixels(), &[Color::rgb(128, 128, 128)]);
    assert_eq!((rgb.width(), rgb.height()), (8, 8));
    assert_eq!(rgb.pixels(), vec![Color::rgb(0, 128, 255); 64]);

    draw_text(
        frame,
        18,
        234,
        "NATIVE JPEG: 12-BIT DISPLAY PRECISION",
        TextStyle::new(
            TEXT,
            TextSize::new(14.0).expect("invariant: 14 px is a valid text size"),
        ),
    );
    for (label, image, bounds, destination, expected) in [
        (
            "GRAY 2048/4095",
            gray,
            Rect::new(18, 270, 180, 24),
            Rect::new(96, 270, 24, 24),
            Color::rgb(128, 128, 128),
        ),
        (
            "RGB 0/2048/4095",
            rgb,
            Rect::new(220, 270, 180, 24),
            Rect::new(298, 270, 24, 24),
            Color::rgb(0, 128, 255),
        ),
    ] {
        render_sample(frame, label, image, bounds, destination, [expected; 2])?;
    }
    Ok(())
}

pub(super) fn render_sample(
    frame: &mut Framebuffer,
    label: &str,
    image: RasterImage,
    bounds: Rect,
    expected_destination: Rect,
    expected_corners: [Color; 2],
) -> Result<(), Box<dyn Error>> {
    draw_text(
        frame,
        bounds.x,
        bounds.y - 18,
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
    let placement = ImagePlacement::contain(image, bounds, ImageTransform::Identity)?;
    assert_eq!(placement.destination(), expected_destination);
    placement.render_to(frame);
    assert_eq!(
        frame.get_pixel(expected_destination.x, expected_destination.y),
        expected_corners[0]
    );
    assert_eq!(
        frame.get_pixel(
            expected_destination.x + expected_destination.width - 1,
            expected_destination.y + expected_destination.height - 1
        ),
        expected_corners[1]
    );
    assert_eq!(frame.get_pixel(bounds.x, bounds.y), PANEL_BACKGROUND);
    assert_eq!(
        frame.get_pixel(bounds.x + bounds.width - 1, bounds.y + bounds.height - 1),
        PANEL_BACKGROUND
    );
    Ok(())
}

fn render_orientation(
    frame: &mut Framebuffer,
    base_blocks: &[Color; 6],
    base_corners: &[Color; 4],
    index: usize,
    label: &str,
) -> Result<(), Box<dyn Error>> {
    let orientation = u8::try_from(index + 1)?;
    let image = RasterImage::decode(&jpeg_with_orientation(orientation)?)?;
    let (expected_width, expected_height, columns, rows) = if orientation >= 5 {
        (32, 48, 2, 3)
    } else {
        (48, 32, 3, 2)
    };
    assert_eq!(
        (image.width(), image.height()),
        (expected_width, expected_height)
    );
    let block_order = BLOCK_ORDERS
        .get(index)
        .expect("invariant: each JPEG label has one block order");
    for (actual, source_index) in block_samples(&image, columns, rows)?
        .into_iter()
        .zip(*block_order)
    {
        let expected = base_blocks
            .get(source_index)
            .expect("invariant: JPEG block order indexes the six source blocks");
        assert_eq!(actual, *expected);
    }

    let column = i32::try_from(index % 4)?;
    let row = i32::try_from(index / 4)?;
    let bounds = Rect::new(
        18 + column * 194,
        346 + row * 104,
        PANEL_WIDTH,
        PANEL_HEIGHT,
    );
    draw_text(
        frame,
        bounds.x,
        bounds.y - 16,
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

    let placement = ImagePlacement::contain(image.clone(), bounds, ImageTransform::Identity)?;
    let expected_destination = if orientation >= 5 {
        Rect::new(bounds.x + 59, bounds.y, 56, 84)
    } else {
        Rect::new(bounds.x + 24, bounds.y, 126, 84)
    };
    assert_eq!(placement.destination(), expected_destination);
    placement.render_to(frame);

    let destination = placement.destination();
    let corner_points = [
        (destination.x, destination.y),
        (destination.x + destination.width - 1, destination.y),
        (destination.x, destination.y + destination.height - 1),
        (
            destination.x + destination.width - 1,
            destination.y + destination.height - 1,
        ),
    ];
    let corner_order = CORNER_ORDERS
        .get(index)
        .expect("invariant: each JPEG label has one corner order");
    for (point, source_index) in corner_points.into_iter().zip(*corner_order) {
        let expected = base_corners
            .get(source_index)
            .expect("invariant: JPEG corner order indexes the four source corners");
        assert_eq!(frame.get_pixel(point.0, point.1), *expected);
    }
    assert_eq!(
        frame.get_pixel(bounds.x, bounds.y + bounds.height / 2),
        PANEL_BACKGROUND
    );
    Ok(())
}
