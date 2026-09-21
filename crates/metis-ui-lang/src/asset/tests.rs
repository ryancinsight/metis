use super::*;
use crate::{ImagePlacement, ImageTransform};
use metis_core::capability::CapabilityGrantSpec;
use metis_core::host::{HostOrigin, HostPolicy, HostSessionId, WindowId};
use metis_platform::{Framebuffer, Rect};

const RGBA: &[u8] = &[
    255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 255, 255, 255, 0, 0, 255, 0, 255, 255, 0, 255, 255,
    255,
];

fn encode(width: u32, height: u32, color: png::ColorType, pixels: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("fixture header");
    writer.write_image_data(pixels).expect("fixture pixels");
    writer.finish().expect("fixture terminal chunk");
    bytes
}

fn fixture() -> Vec<u8> {
    encode(2, 3, png::ColorType::Rgba, RGBA)
}

fn compressed_png(payloads: &[&[u8]], interlaced: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut info = png::Info::with_size(2, 3);
    info.color_type = png::ColorType::Rgba;
    info.bit_depth = png::BitDepth::Eight;
    info.interlaced = interlaced;
    let encoder = png::Encoder::with_info(&mut bytes, info).expect("fixture descriptor");
    let mut writer = encoder.write_header().expect("fixture header");
    for payload in payloads {
        writer
            .write_chunk(png::chunk::IDAT, payload)
            .expect("fixture compressed chunk");
    }
    writer.finish().expect("fixture end");
    bytes
}

fn compress(raw: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(raw).expect("fixture scanlines");
    encoder.finish().expect("fixture zlib end")
}

#[test]
fn exact_zlib_end_checksum_and_scanline_extent_are_required() {
    let rows: Vec<_> = RGBA
        .chunks_exact(8)
        .flat_map(|row| std::iter::once(0).chain(row.iter().copied()))
        .collect();
    let compressed = compress(&rows);
    let expected = RasterImage::decode(&fixture()).expect("reference");
    for split in 0..=compressed.len() {
        assert_eq!(
            RasterImage::decode(&compressed_png(
                &[&compressed[..split], &compressed[split..]],
                false
            ))
            .expect("split stream"),
            expected
        );
    }
    for end in 0..compressed.len() {
        rejected(
            &compressed_png(&[&compressed[..end]], false),
            AssetErrorKind::Malformed,
        );
    }
    let mut bad_adler = compressed.clone();
    *bad_adler.last_mut().expect("checksum") ^= 1;
    rejected(
        &compressed_png(&[&bad_adler], false),
        AssetErrorKind::Malformed,
    );
    rejected(
        &compressed_png(&[&compressed, &[0]], false),
        AssetErrorKind::Malformed,
    );
    let mut excess = rows;
    excess.push(0);
    rejected(
        &compressed_png(&[&compress(&excess)], false),
        AssetErrorKind::Malformed,
    );
}

#[test]
fn adam7_scanlines_preserve_the_same_rgba_grid() {
    // Independent 2x3 Adam7 traversal: A; E; B; F; C,D. Each pass row
    // begins with filter zero; empty passes emit neither pixels nor filters.
    let mut raw = Vec::new();
    for indices in [&[0][..], &[4], &[1], &[5], &[2, 3]] {
        raw.push(0);
        for &index in indices {
            raw.extend_from_slice(&RGBA[index * 4..index * 4 + 4]);
        }
    }
    assert_eq!(raw.len(), 29);
    let image =
        RasterImage::decode(&compressed_png(&[&compress(&raw)], true)).expect("Adam7 decode");
    assert_eq!(
        image,
        RasterImage::decode(&fixture()).expect("row-major decode")
    );
}

fn rejected(bytes: &[u8], kind: AssetErrorKind) {
    assert_eq!(
        RasterImage::decode(bytes)
            .expect_err("rejected fixture")
            .kind(),
        kind
    );
}

#[test]
fn decoded_alpha_orientation_and_contained_pixels_match_fixture() {
    let image = RasterImage::decode(&fixture()).expect("decode fixture");
    assert_eq!((image.width(), image.height()), (2, 3));
    let expected: Vec<_> = RGBA
        .chunks_exact(4)
        .map(|sample| Color::rgba(sample[0], sample[1], sample[2], sample[3]))
        .collect();
    assert_eq!(image.pixels(), expected);
    let placement = ImagePlacement::contain(
        image,
        Rect::new(0, 0, 8, 8),
        ImageTransform::RotateClockwise,
    )
    .expect("contained quarter turn");
    assert_eq!(placement.destination(), Rect::new(0, 1, 8, 5));
    let mut frame = Framebuffer::new(8, 8).expect("frame");
    frame.clear(Color::BLACK);
    placement.render_to(&mut frame);
    assert_eq!(frame.get_pixel(0, 0), Color::BLACK);
    assert_eq!(frame.get_pixel(0, 1), Color::rgb(255, 0, 255));
    assert_eq!(frame.get_pixel(3, 1), Color::rgb(0, 0, 255));
    assert_eq!(frame.get_pixel(7, 1), Color::rgb(255, 0, 0));
    assert_eq!(frame.get_pixel(0, 5), Color::rgb(0, 255, 255));
    assert_eq!(frame.get_pixel(3, 5), Color::BLACK);
    // Integer source-over over opaque black: (255 * 128 + 127) / 255 = 128.
    assert_eq!(frame.get_pixel(7, 5), Color::rgb(0, 128, 0));
    assert_eq!(frame.get_pixel(0, 6), Color::BLACK);
}

#[test]
fn admitted_color_forms_expand_to_exact_straight_channels() {
    for (color, input, expected) in [
        (
            png::ColorType::Rgb,
            vec![10, 20, 30],
            Color::rgb(10, 20, 30),
        ),
        (png::ColorType::Grayscale, vec![41], Color::rgb(41, 41, 41)),
        (
            png::ColorType::GrayscaleAlpha,
            vec![41, 17],
            Color::rgba(41, 41, 41, 17),
        ),
    ] {
        let image = RasterImage::decode(&encode(1, 1, color, &input)).expect("color fixture");
        assert_eq!(image.pixels(), &[expected]);
    }
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_depth(png::BitDepth::One);
    encoder.set_palette(vec![255, 0, 0, 0, 0, 255]);
    encoder.set_trns(vec![255, 64]);
    let mut writer = encoder.write_header().expect("palette header");
    writer
        .write_image_data(&[0b0100_0000])
        .expect("palette pixels");
    writer.finish().expect("palette end");
    assert_eq!(
        RasterImage::decode(&bytes)
            .expect("palette decode")
            .pixels(),
        &[Color::rgb(255, 0, 0), Color::rgba(0, 0, 255, 64)]
    );
}

#[test]
fn every_truncation_and_single_byte_corruption_fails() {
    let bytes = fixture();
    for end in 0..bytes.len() {
        rejected(&bytes[..end], AssetErrorKind::Malformed);
    }
    for index in 0..bytes.len() {
        let mut corrupted = bytes.clone();
        corrupted[index] ^= 1;
        let error = RasterImage::decode(&corrupted).expect_err("corrupted byte");
        assert!(matches!(
            error.kind(),
            AssetErrorKind::Malformed | AssetErrorKind::Unsupported | AssetErrorKind::TooLarge
        ));
    }
    let mut trailing = bytes;
    trailing.push(0);
    rejected(&trailing, AssetErrorKind::Malformed);
}

#[test]
fn dimensions_encoded_bytes_and_metadata_fail_before_pixel_allocation() {
    for (width, height) in [(MAX_IMAGE_DIMENSION + 1, 1), (4097, 4096)] {
        let mut bytes = Vec::new();
        let encoder = png::Encoder::new(&mut bytes, width, height);
        let mut writer = encoder.write_header().expect("oversized header");
        writer
            .write_chunk(png::chunk::IDAT, &[0])
            .expect("unreachable image data");
        writer.finish().expect("end");
        rejected(&bytes, AssetErrorKind::TooLarge);
    }
    rejected(
        &vec![0; MAX_ENCODED_IMAGE_BYTES + 1],
        AssetErrorKind::TooLarge,
    );
    for chunk in [*b"pHYs", *b"iCCP", *b"acTL", *b"zTXt"] {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        let mut writer = encoder.write_header().expect("metadata header");
        writer
            .write_chunk(png::chunk::ChunkType(chunk), &[0; 8])
            .expect("metadata chunk");
        writer.write_image_data(&[0; 4]).expect("pixel");
        writer.finish().expect("end");
        rejected(&bytes, AssetErrorKind::Unsupported);
    }
}

fn capability() -> VerifiedHostCapability<{ CapabilityScope::READ_FILE.0 }> {
    let key = b"asset-test-signing-key";
    let policy = HostPolicy::new(
        HostOrigin::parse("http://127.0.0.1:8080").expect("origin"),
        WindowId::new(1).expect("window"),
    );
    let session = HostSessionId::new([7; 16]).expect("session");
    let context = policy.context_for(session);
    let token = context
        .issue_capability(
            CapabilityGrantSpec {
                token_id: 1,
                principal_id: session.as_bytes(),
                scope: CapabilityScope::READ_FILE,
                issued_at_secs: 10,
                duration_secs: 100,
                nonce: 1,
            },
            key,
        )
        .expect("token");
    policy
        .authorize(&token, &context, 11, key)
        .expect("file witness")
}

#[test]
fn scoped_loader_rejects_parent_absolute_stream_and_oversized_files() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("metis-assets-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&root).expect("fixture root");
    std::fs::write(root.join("image.png"), fixture()).expect("fixture file");
    let provider = ScopedFileProvider::new(&root).expect("root");
    let capability = capability();
    assert_eq!(
        RasterImage::load(&provider, &capability, "image.png")
            .expect("scoped decode")
            .pixels()[0],
        Color::rgb(255, 0, 0)
    );
    let jpeg = jpeg_cases::jpeg(false);
    std::fs::write(root.join("image.bin"), &jpeg).expect("JPEG fixture file");
    assert_eq!(
        RasterImage::load(&provider, &capability, "image.bin").expect("signature decode"),
        RasterImage::decode(&jpeg).expect("memory decode")
    );
    for path in [
        root.join("image.png"),
        "../image.png".into(),
        "image.png:payload".into(),
        "missing.png".into(),
    ] {
        assert_eq!(
            RasterImage::load(&provider, &capability, path)
                .expect_err("denied path")
                .kind(),
            AssetErrorKind::Access
        );
    }
    let file = std::fs::File::create(root.join("large.png")).expect("sparse file");
    file.set_len(u64::try_from(MAX_ENCODED_IMAGE_BYTES).expect("byte cap") + 1)
        .expect("oversize length");
    drop(file);
    assert_eq!(
        RasterImage::load(&provider, &capability, "large.png")
            .expect_err("oversize file")
            .kind(),
        AssetErrorKind::Access
    );
    std::fs::remove_dir_all(root).expect("fixture cleanup");
}

mod jpeg_cases;
