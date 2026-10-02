//! Allocation accounting for PNG decode.
//!
//! A counting global allocator records the allocations of the measuring
//! thread that are at least one decoded image in size. The counters are
//! thread-local, so the scenarios are independent tests that neither share
//! nor corrupt each other's counts.
#![cfg(not(target_arch = "wasm32"))]

use metis_platform::Color;
use metis_ui_lang::RasterImage;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

const SIDE: u32 = 2048;
const IMAGE_BYTES: usize = SIDE as usize * SIDE as usize * 4;
/// Bytes of the smallest separate sample buffer a two-pass decode would add:
/// one byte per pixel, the grayscale case. The decoder's own fixed inflate and
/// scanline buffers do not scale with the image and stay far below it (about
/// 275 KiB measured with png 0.18.1), so a peak under one image grid plus this
/// margin admits them and rejects any per-pixel staging buffer.
const SMALLEST_SEPARATE_BUFFER_BYTES: usize = SIDE as usize * SIDE as usize;

#[derive(Clone, Copy)]
struct Counters {
    live: usize,
    peak: usize,
    large_allocations: usize,
    large_threshold: usize,
    /// Set when a release exceeds the live total, meaning memory allocated
    /// before the measurement was freed inside it and the peak is unsound.
    released_unmeasured_memory: bool,
}

thread_local! {
    // Constant-initialised and destructor-free, so the allocator can touch it
    // without allocating.
    static COUNTERS: Cell<Counters> = const {
        Cell::new(Counters {
            live: 0,
            peak: 0,
            large_allocations: 0,
            large_threshold: usize::MAX,
            released_unmeasured_memory: false,
        })
    };
}

struct Counting;

impl Counting {
    fn update(release: usize, acquire: usize) {
        let updated = COUNTERS.try_with(|cell| {
            let mut counters = cell.get();
            if let Some(live) = counters.live.checked_sub(release) {
                counters.live = live + acquire;
            } else {
                counters.released_unmeasured_memory = true;
                counters.live = acquire;
            }
            counters.peak = counters.peak.max(counters.live);
            if acquire >= counters.large_threshold {
                counters.large_allocations += 1;
            }
            cell.set(counters);
        });
        match updated {
            Ok(()) => {}
            // A thread already torn down has no counters to update.
            Err(_torn_down) => {}
        }
    }
}

#[expect(
    unsafe_code,
    reason = "GlobalAlloc is an unsafe trait; this counting allocator forwards every call to System unchanged"
)]
// SAFETY: every method forwards its arguments unchanged to `System`, which
// upholds the `GlobalAlloc` contract; the counters are thread-local side data.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's layout obligations pass through to `System`.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            Self::update(0, layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        Self::update(layout.size(), 0);
        // SAFETY: `pointer` and `layout` are the caller's allocation pair.
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: `pointer`, `layout` and `new_size` are the caller's arguments.
        let resized = unsafe { System.realloc(pointer, layout, new_size) };
        if !resized.is_null() {
            Self::update(layout.size(), new_size);
        }
        resized
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[derive(Debug, PartialEq, Eq)]
struct Measurement {
    large_allocations: usize,
    peak_bytes: usize,
}

fn measure<T>(work: impl FnOnce() -> T) -> (T, Measurement) {
    let baseline = COUNTERS.get().live;
    COUNTERS.set(Counters {
        live: baseline,
        peak: baseline,
        large_allocations: 0,
        large_threshold: IMAGE_BYTES,
        released_unmeasured_memory: false,
    });
    let value = work();
    let counters = COUNTERS.get();
    COUNTERS.set(Counters {
        large_threshold: usize::MAX,
        ..counters
    });
    assert!(
        !counters.released_unmeasured_memory,
        "memory allocated before the measurement was freed inside it"
    );
    let measurement = Measurement {
        large_allocations: counters.large_allocations,
        peak_bytes: counters.peak - baseline,
    };
    (value, measurement)
}

/// Rows repeat with this period, so fixtures and expectations are built from
/// a few rows and copied by slice; every row still differs from its
/// neighbours, which distinguishes a mirrored or transposed grid.
const ROW_PERIOD: u32 = 16;

fn sample(column: u32, row: u32, channel: u32) -> u8 {
    column
        .wrapping_mul(7)
        .wrapping_add(row.wrapping_mul(13))
        .wrapping_add(channel.wrapping_mul(31))
        .to_le_bytes()[0]
}

fn raw_samples(channels: u32) -> Vec<u8> {
    let rows: Vec<Vec<u8>> = (0..ROW_PERIOD)
        .map(|row| {
            (0..SIDE)
                .flat_map(|x| (0..channels).map(move |channel| sample(x, row, channel)))
                .collect()
        })
        .collect();
    let mut raw = Vec::with_capacity((SIDE * SIDE * channels) as usize);
    for y in 0..SIDE {
        raw.extend_from_slice(&rows[(y % ROW_PERIOD) as usize]);
    }
    raw
}

fn tiff_orientation(orientation: u16) -> Vec<u8> {
    let mut bytes = b"II".to_vec();
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

fn encode(color: png::ColorType, orientation: Option<u16>) -> Vec<u8> {
    let raw = raw_samples(u32::try_from(color.samples()).expect("sample count fits u32"));
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, SIDE, SIDE);
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    // Stored blocks keep fixture construction cheap; the decoder still runs
    // its full inflate and checksum path over every byte.
    encoder.set_compression(png::Compression::NoCompression);
    encoder.set_filter(png::Filter::NoFilter);
    let mut writer = encoder.write_header().expect("PNG header");
    if let Some(value) = orientation {
        writer
            .write_chunk(png::chunk::ChunkType(*b"eXIf"), &tiff_orientation(value))
            .expect("PNG EXIF chunk");
    }
    writer.write_image_data(&raw).expect("PNG pixels");
    writer.finish().expect("PNG end");
    bytes
}

fn expected(color: png::ColorType, column: u32, row: u32) -> Color {
    let channel = |index| sample(column, row, index);
    match color {
        png::ColorType::Grayscale => Color::rgb(channel(0), channel(0), channel(0)),
        png::ColorType::GrayscaleAlpha => {
            Color::rgba(channel(0), channel(0), channel(0), channel(1))
        }
        png::ColorType::Rgb => Color::rgb(channel(0), channel(1), channel(2)),
        png::ColorType::Rgba => Color::rgba(channel(0), channel(1), channel(2), channel(3)),
        png::ColorType::Indexed => unreachable!("indexed images are not generated"),
    }
}

fn assert_pixels(image: &RasterImage, color: png::ColorType, mirrored: bool) {
    assert_eq!((image.width(), image.height()), (SIDE, SIDE));
    let rows: Vec<Vec<Color>> = (0..ROW_PERIOD)
        .map(|row| {
            (0..SIDE)
                .map(|x| expected(color, if mirrored { SIDE - 1 - x } else { x }, row))
                .collect()
        })
        .collect();
    let mut decoded_rows = image.pixels().chunks_exact(SIDE as usize);
    for y in 0..SIDE {
        let matches = decoded_rows
            .next()
            .is_some_and(|row| row == rows[(y % ROW_PERIOD) as usize]);
        assert!(matches, "row {y} of {color:?}, mirrored: {mirrored}");
    }
    assert_eq!(decoded_rows.next(), None);
}

/// Without rotation the fallibly reserved decode buffer is the stored image;
/// a rotation reads a source grid while writing its destination, so exactly one
/// further grid is live until the source drops.
fn assert_single_pass(color: png::ColorType, orientation: Option<u16>) {
    let encoded = encode(color, orientation);
    let (decoded, measurement) = measure(|| RasterImage::decode(&encoded));
    let image = decoded.expect("decode admitted PNG");
    let grids = if orientation.is_some() { 2 } else { 1 };
    assert_eq!(
        measurement.large_allocations, grids,
        "{color:?} exif {orientation:?}: allocations of one image or more, {measurement:?}"
    );
    assert!(
        measurement.peak_bytes < grids * IMAGE_BYTES + SMALLEST_SEPARATE_BUFFER_BYTES,
        "{color:?} exif {orientation:?}: peak reaches {grids} grid(s) plus a per-pixel buffer, {measurement:?}"
    );
    assert_pixels(&image, color, orientation.is_some());
}

#[test]
fn rgba_decode_retains_one_image_buffer() {
    assert_single_pass(png::ColorType::Rgba, None);
}

#[test]
fn rgb_decode_expands_in_the_image_buffer() {
    assert_single_pass(png::ColorType::Rgb, None);
}

#[test]
fn gray_decode_expands_in_the_image_buffer() {
    assert_single_pass(png::ColorType::Grayscale, None);
}

#[test]
fn gray_alpha_decode_expands_in_the_image_buffer() {
    assert_single_pass(png::ColorType::GrayscaleAlpha, None);
}

#[test]
fn oriented_decode_holds_source_and_destination_only() {
    assert_single_pass(png::ColorType::Rgba, Some(2));
}
