use super::super::{AssetErrorKind, MAX_IMAGE_PIXELS};
use crate::RasterImage;
use jpeg_encoder::{ColorType, Encoder};
use metis_platform::Color;

const WIDTH: u16 = 2;
const HEIGHT: u16 = 3;

fn source_pixels() -> Vec<u8> {
    vec![
        230, 25, 35, 30, 210, 50, 25, 55, 225, 240, 185, 25, 30, 205, 215, 215, 35, 195,
    ]
}

pub(super) fn jpeg(progressive: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes, 100);
    encoder.set_progressive(progressive);
    encoder
        .encode(&source_pixels(), WIDTH, HEIGHT, ColorType::Rgb)
        .expect("JPEG fixture");
    bytes
}

fn tiff(orientation: u16) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"II");
    bytes.extend_from_slice(&42_u16.to_le_bytes());
    bytes.extend_from_slice(&8_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&0x0112_u16.to_le_bytes());
    bytes.extend_from_slice(&3_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&orientation.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes
}

fn with_exif(jpeg: &[u8], orientation: u16) -> Vec<u8> {
    let mut payload = b"Exif\0\0".to_vec();
    payload.extend_from_slice(&tiff(orientation));
    let length = u16::try_from(payload.len() + 2).expect("small EXIF fixture");
    let mut output = Vec::with_capacity(jpeg.len() + payload.len() + 4);
    output.extend_from_slice(&jpeg[..2]);
    output.extend_from_slice(&[0xff, 0xe1]);
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(&payload);
    output.extend_from_slice(&jpeg[2..]);
    output
}

fn expected_orientation(source: &RasterImage, orientation: u16) -> (u32, u32, Vec<Color>) {
    const ORDERS: [[usize; 6]; 8] = [
        [0, 1, 2, 3, 4, 5],
        [1, 0, 3, 2, 5, 4],
        [5, 4, 3, 2, 1, 0],
        [4, 5, 2, 3, 0, 1],
        [0, 2, 4, 1, 3, 5],
        [4, 2, 0, 5, 3, 1],
        [5, 3, 1, 4, 2, 0],
        [1, 3, 5, 0, 2, 4],
    ];
    let source: [Color; 6] = source
        .pixels()
        .try_into()
        .expect("fixture is an exact 2x3 pixel grid");
    let order_index = usize::from(
        orientation
            .checked_sub(1)
            .expect("fixture orientations are one through eight"),
    );
    let order = ORDERS
        .get(order_index)
        .expect("fixture orientation has a source order");
    let pixels = order.map(|index| {
        *source
            .get(index)
            .expect("fixture orientation order indexes six source pixels")
    });
    let (width, height) = if orientation >= 5 { (3, 2) } else { (2, 3) };
    (width, height, pixels.into())
}

#[test]
fn baseline_and_progressive_jpeg_normalize_all_exif_orientations() {
    for progressive in [false, true] {
        let encoded = jpeg(progressive);
        let source = RasterImage::decode(&encoded).expect("unoriented JPEG");
        assert_eq!(
            (source.width(), source.height()),
            (u32::from(WIDTH), u32::from(HEIGHT))
        );
        assert!(source.pixels().iter().all(|pixel| pixel.a == 255));
        for value in 1..=8 {
            let image = RasterImage::decode(&with_exif(&encoded, value)).expect("oriented JPEG");
            let (width, height, pixels) = expected_orientation(&source, value);
            assert_eq!((image.width(), image.height()), (width, height));
            assert_eq!(image.pixels(), pixels);
        }
    }
}

#[test]
fn png_exif_uses_the_same_eight_orientation_contracts_with_alpha() {
    let source = [
        Color::rgba(255, 0, 0, 255),
        Color::rgba(0, 255, 0, 128),
        Color::rgba(0, 0, 255, 64),
        Color::rgba(255, 255, 0, 0),
        Color::rgba(0, 255, 255, 200),
        Color::rgba(255, 0, 255, 17),
    ];
    let source_image = RasterImage::new(2, 3, source).expect("source grid");
    let rgba: Vec<_> = source
        .iter()
        .flat_map(|pixel| [pixel.r, pixel.g, pixel.b, pixel.a])
        .collect();
    for value in 1..=8 {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 2, 3);
        encoder.set_color(png::ColorType::Rgba);
        let mut writer = encoder.write_header().expect("PNG header");
        writer
            .write_chunk(png::chunk::ChunkType(*b"eXIf"), &tiff(value))
            .expect("PNG EXIF");
        writer.write_image_data(&rgba).expect("PNG pixels");
        writer.finish().expect("PNG end");
        let image = RasterImage::decode(&bytes).expect("oriented PNG");
        let (width, height, expected) = expected_orientation(&source_image, value);
        assert_eq!((image.width(), image.height()), (width, height));
        assert_eq!(image.pixels(), expected);
    }
}

#[test]
fn every_jpeg_prefix_and_in_scan_reterminated_cut_is_rejected() {
    for progressive in [false, true] {
        let bytes = jpeg(progressive);
        for end in 2..bytes.len() {
            assert_eq!(
                RasterImage::decode(&bytes[..end])
                    .expect_err("truncated JPEG")
                    .kind(),
                AssetErrorKind::Malformed,
                "accepted prefix ending at {end}"
            );
        }
        for (start, end) in entropy_ranges(&bytes) {
            for cut_at in [start, start + (end - start) / 2, end.saturating_sub(1)] {
                if cut_at >= end {
                    continue;
                }
                let mut cut = bytes[..cut_at].to_vec();
                cut.extend_from_slice(&[0xff, 0xd9]);
                assert_eq!(
                    RasterImage::decode(&cut)
                        .expect_err("truncated JPEG entropy")
                        .kind(),
                    AssetErrorKind::Malformed,
                    "accepted reterminated entropy at {cut_at}"
                );
            }
        }
        if !progressive {
            for end in 2..bytes.len() - 2 {
                let mut cut = bytes[..end].to_vec();
                cut.extend_from_slice(&[0xff, 0xd9]);
                let error = RasterImage::decode(&cut).expect_err("reterminated JPEG cut");
                assert_eq!(
                    error.kind(),
                    AssetErrorKind::Malformed,
                    "cut ending at {end}"
                );
            }
        }
    }
}

fn entropy_ranges(bytes: &[u8]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut cursor = 2;
    while cursor + 1 < bytes.len() {
        while bytes[cursor] == 0xff {
            cursor += 1;
        }
        let marker = bytes[cursor];
        cursor += 1;
        if marker == 0xd9 {
            break;
        }
        let length = usize::from(u16::from_be_bytes([bytes[cursor], bytes[cursor + 1]]));
        let is_scan = marker == 0xda;
        cursor += length;
        if !is_scan {
            continue;
        }
        let start = cursor;
        loop {
            let relative = bytes[cursor..]
                .iter()
                .position(|byte| *byte == 0xff)
                .expect("encoded fixture terminal marker");
            let marker_start = cursor + relative;
            let code = bytes[marker_start + 1];
            if code == 0 || matches!(code, 0xd0..=0xd7) {
                cursor = marker_start + 2;
            } else {
                ranges.push((start, marker_start));
                cursor = marker_start;
                break;
            }
        }
    }
    ranges
}

#[test]
fn arbitrary_and_mutated_bytes_never_escape_image_bounds() {
    let reference = jpeg(false);
    let mut state = 0x9e37_79b9_u32;
    for length in 0..256 {
        let mut bytes = Vec::with_capacity(length);
        for _ in 0..length {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            bytes.push(state.to_le_bytes()[0]);
        }
        assert_bounded(RasterImage::decode(&bytes));
    }
    for index in 0..reference.len() {
        let mut bytes = reference.clone();
        bytes[index] ^= 0x5a;
        assert_bounded(RasterImage::decode(&bytes));
    }
}

fn assert_bounded(result: Result<RasterImage, super::super::AssetError>) {
    match result {
        Ok(image) => {
            assert!(image.pixels().len() <= MAX_IMAGE_PIXELS);
            assert!(image.pixels().iter().all(|pixel| pixel.a == 255));
        }
        Err(error) => assert!(matches!(
            error.kind(),
            AssetErrorKind::Malformed
                | AssetErrorKind::Unsupported
                | AssetErrorKind::TooLarge
                | AssetErrorKind::Allocation
        )),
    }
}

#[test]
fn grayscale_and_cmyk_decode_to_opaque_rgb() {
    for (sample, expected) in [(0, Color::rgb(0, 0, 0)), (255, Color::rgb(255, 255, 255))] {
        let mut gray = Vec::new();
        Encoder::new(&mut gray, 100)
            .encode(&[sample; 64], 8, 8, ColorType::Luma)
            .expect("grayscale JPEG");
        let gray = RasterImage::decode(&gray).expect("grayscale decode");
        assert_eq!(gray.pixels(), &[expected; 64]);
    }

    let mut cmyk = Vec::new();
    Encoder::new(&mut cmyk, 100)
        .encode(&[255, 0, 0, 0].repeat(64), 8, 8, ColorType::Cmyk)
        .expect("CMYK JPEG");
    let cmyk = RasterImage::decode(&cmyk).expect("CMYK decode");
    assert_eq!(cmyk.pixels(), &[Color::rgb(0, 255, 255); 64]);
}

#[test]
fn lossless_samples_preserve_display_precision_boundary() {
    // T.81 SOF3, predictor 1, one category-zero sample: 2^(8-1) = 128.
    let mut bytes = vec![
        0xff, 0xd8, 0xff, 0xc3, 0, 11, 8, 0, 1, 0, 1, 1, 1, 0x11, 0, 0xff, 0xc4, 0, 20, 0, 1, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xda, 0, 8, 1, 1, 0, 1, 0, 0, 0x7f, 0xff,
        0xd9,
    ];
    let image = RasterImage::decode(&bytes).expect("lossless display sample");
    assert_eq!((image.width(), image.height()), (1, 1));
    assert_eq!(image.pixels(), &[Color::rgb(128, 128, 128)]);
    bytes[6] = 16;
    assert_eq!(
        RasterImage::decode(&bytes)
            .expect_err("wide samples need explicit display mapping")
            .kind(),
        AssetErrorKind::Unsupported,
    );
}

#[test]
fn independent_sequential_and_refined_progressive_fixtures_decode_identically() {
    // Pillow 12.1.1 generated both files at quality 90 with 4:2:0 sampling from
    // RGB(x, y) = ((13x + 3y) mod 256, (5x + 7y) mod 256, (11x + 17y) mod 256).
    let sequential = include_bytes!("fixtures/sequential.jpg");
    let progressive = include_bytes!("fixtures/progressive.jpg");
    let sequential = RasterImage::decode(sequential).expect("sequential JPEG");
    let progressive = RasterImage::decode(progressive).expect("progressive JPEG");
    assert_eq!((sequential.width(), sequential.height()), (17, 25));
    assert_eq!(progressive, sequential);
    assert!(progressive.pixels().iter().all(|pixel| pixel.a == 255));
}
