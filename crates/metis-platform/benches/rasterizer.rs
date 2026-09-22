//! Software rasterizer fill and glyph throughput on a fixed presentation extent.
//!
//! The extent matches the 1280x800 physical surface used by the V12 resource
//! fixtures, so a measured ratio here maps onto a real presentation repaint.
//! Every case reuses one framebuffer: composite cost per pixel is independent
//! of the destination values, so accumulated blending does not move the timing.

use criterion::{Criterion, criterion_group, criterion_main};
use metis_platform::framebuffer::{Color, Framebuffer, Rect};
use metis_platform::rasterizer::{CornerRadius, draw_rect_outline, draw_text, fill_rect};
use metis_platform::{GlyphWeight, TextStyle};
use std::hint::black_box;

/// Physical width of the measured presentation surface.
const WIDTH: u32 = 1280;
/// Physical height of the measured presentation surface.
const HEIGHT: u32 = 800;
/// Card count approximating one authored form repaint.
const CARDS: i32 = 8;

fn surface() -> Framebuffer {
    Framebuffer::new(WIDTH, HEIGHT).expect("invariant: fixture extent is within MAX_PIXELS")
}

fn fills(c: &mut Criterion) {
    let mut group = c.benchmark_group("fill");
    let fits = "invariant: the fixture extent fits a signed coordinate";
    let full = Rect::new(
        0,
        0,
        i32::try_from(WIDTH).expect(fits),
        i32::try_from(HEIGHT).expect(fits),
    );

    let mut opaque = surface();
    group.bench_function("opaque_full_surface", |b| {
        b.iter(|| {
            fill_rect(
                &mut opaque,
                black_box(full),
                CornerRadius::SQUARE,
                black_box(Color::DARK_BLUE),
            );
            opaque.get_pixel(0, 0)
        });
    });

    let mut translucent = surface();
    translucent.clear(Color::WHITE);
    group.bench_function("translucent_full_surface", |b| {
        b.iter(|| {
            fill_rect(
                &mut translucent,
                black_box(full),
                CornerRadius::SQUARE,
                black_box(Color::rgba(49, 130, 206, 128)),
            );
            translucent.get_pixel(0, 0)
        });
    });

    let mut cards = surface();
    group.bench_function("card_stack", |b| {
        b.iter(|| {
            for index in 0..CARDS {
                let rect = Rect::new(24, 24 + index * 90, 520, 72);
                fill_rect(
                    &mut cards,
                    black_box(rect),
                    CornerRadius::SQUARE,
                    black_box(Color::WHITE),
                );
                draw_rect_outline(
                    &mut cards,
                    black_box(rect),
                    1,
                    CornerRadius::SQUARE,
                    black_box(Color::LIGHT_GRAY),
                );
            }
            cards.get_pixel(24, 24)
        });
    });
    let mut rounded = surface();
    group.bench_function("rounded_card_stack", |b| {
        b.iter(|| {
            for index in 0..CARDS {
                let rect = Rect::new(24, 24 + index * 90, 520, 72);
                let radius = CornerRadius::clamped(12, rect);
                fill_rect(
                    &mut rounded,
                    black_box(rect),
                    radius,
                    black_box(Color::WHITE),
                );
                draw_rect_outline(
                    &mut rounded,
                    black_box(rect),
                    1,
                    radius,
                    black_box(Color::LIGHT_GRAY),
                );
            }
            rounded.get_pixel(24, 24)
        });
    });
    group.finish();
}

fn text(c: &mut Criterion) {
    let mut group = c.benchmark_group("text");
    let label = "Patient weight 72.5 kg / concentration 1.2 mg per ml";

    let mut single = surface();
    group.bench_function("label_scale_one", |b| {
        b.iter(|| {
            draw_text(
                &mut single,
                black_box(16),
                black_box(16),
                black_box(label),
                black_box(TextStyle::new(Color::BLACK, 1).with_weight(GlyphWeight::Regular)),
            );
            single.get_pixel(16, 16)
        });
    });

    let mut paragraph = surface();
    group.bench_function("paragraph_scale_two", |b| {
        b.iter(|| {
            for row in 0..16 {
                draw_text(
                    &mut paragraph,
                    black_box(16),
                    black_box(16 + row * 34),
                    black_box(label),
                    black_box(TextStyle::new(Color::BLACK, 2).with_weight(GlyphWeight::Regular)),
                );
            }
            paragraph.get_pixel(16, 16)
        });
    });
    let mut bold = surface();
    group.bench_function("label_bold", |b| {
        b.iter(|| {
            draw_text(
                &mut bold,
                black_box(16),
                black_box(16),
                black_box(label),
                black_box(TextStyle::new(Color::BLACK, 1).with_weight(GlyphWeight::Bold)),
            );
            bold.get_pixel(16, 16)
        });
    });
    group.finish();
}

criterion_group! {
    name = rasterizer;
    // A committed instrument budget: five cases at roughly two seconds each
    // stay far inside the repository's suite-total wall-clock bound.
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_millis(500))
        .measurement_time(std::time::Duration::from_secs(2))
        .sample_size(20);
    targets = fills, text
}
criterion_main!(rasterizer);
