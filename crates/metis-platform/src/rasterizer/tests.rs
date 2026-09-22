use super::*;
use crate::DisplayScale;
use crate::font::{GlyphWeight, draw_glyph};

/// Deterministic xorshift source so a differential failure replays exactly.
struct Sequence(u64);

impl Sequence {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn in_range(&mut self, low: i32, high: i32) -> i32 {
        let span = u64::from(high.abs_diff(low)) + 1;
        let offset = i32::try_from(self.next() % span).expect("bounded span fits i32");
        low + offset
    }

    fn color(&mut self) -> Color {
        let bits = self.next();
        let channel =
            |shift: u32| u8::try_from((bits >> shift) & 0xff).expect("byte extracted from a word");
        Color::rgba(channel(0), channel(8), channel(16), channel(24))
    }
}

/// Composites a rectangle one pixel at a time, the contract span filling
/// must reproduce exactly.
fn blend_each_pixel(fb: &mut Framebuffer, rect: Rect, color: Color) {
    for row in 0..rect.height {
        for column in 0..rect.width {
            fb.blend_pixel(
                rect.x.saturating_add(column),
                rect.y.saturating_add(row),
                color,
            );
        }
    }
}

#[test]
fn span_fill_matches_per_pixel_compositing_for_random_rectangles() {
    let mut sequence = Sequence(0x2545_f491_4f6c_dd1d);
    for case in 0..512 {
        let mut spans = Framebuffer::new(37, 23).expect("differential surface");
        let mut pixels = Framebuffer::new(37, 23).expect("reference surface");
        // Alternate a transparent and an opaque starting surface so both
        // destination-alpha regimes take part in the comparison.
        if case % 2 == 0 {
            spans.clear(Color::rgba(17, 200, 91, 203));
            pixels.clear(Color::rgba(17, 200, 91, 203));
        }
        for _ in 0..4 {
            let rect = Rect::new(
                sequence.in_range(-8, 40),
                sequence.in_range(-8, 26),
                sequence.in_range(0, 44),
                sequence.in_range(0, 30),
            );
            let color = sequence.color();
            fill_rect(&mut spans, rect, CornerRadius::SQUARE, color);
            blend_each_pixel(&mut pixels, rect, color);
        }
        assert_eq!(
            spans.pixels(),
            pixels.pixels(),
            "case {case} diverged from per-pixel compositing"
        );
    }
}

#[test]
fn glyph_runs_match_per_pixel_cell_compositing() {
    for character in ['A', 'W', '8', '%', '_', ' ', '\u{1f4a5}'] {
        for scale in [1_i32, 2, 3] {
            let mut runs = Framebuffer::new(40, 60).expect("glyph surface");
            let mut cells = Framebuffer::new(40, 60).expect("cell surface");
            runs.clear(Color::WHITE);
            cells.clear(Color::WHITE);
            let color = Color::rgba(20, 60, 180, 137);
            let requested = u32::try_from(scale).expect("small scale fits u32");
            draw_glyph(
                &mut runs,
                3,
                5,
                character,
                TextStyle::new(color, requested).with_weight(GlyphWeight::Regular),
            );
            for (row, byte) in (0_i32..16).zip(crate::font::get_glyph_bitmap(character)) {
                for column in 0_i32..8 {
                    if byte & (0x80_u8 >> column) == 0 {
                        continue;
                    }
                    blend_each_pixel(
                        &mut cells,
                        Rect::new(3 + column * scale, 5 + row * scale, scale, scale),
                        color,
                    );
                }
            }
            assert_eq!(
                runs.pixels(),
                cells.pixels(),
                "glyph {character:?} at scale {scale} diverged from per-cell compositing"
            );
        }
    }
}

#[test]
fn extreme_geometry_clips_without_overflow() {
    let mut fb = Framebuffer::new(2, 2).expect("small surface");
    fill_rect(
        &mut fb,
        Rect::new(-1, -1, i32::MAX, i32::MAX),
        CornerRadius::SQUARE,
        Color::GREEN,
    );
    assert_eq!(fb.pixels(), &[0xff38_a169; 4]);
    draw_text(
        &mut fb,
        i32::MAX,
        i32::MIN,
        "A",
        TextStyle::new(Color::WHITE, u32::MAX).with_weight(GlyphWeight::Regular),
    );
    assert_eq!(fb.get_pixel(0, 0), Color::GREEN);
    draw_glyph(
        &mut fb,
        0,
        0,
        'A',
        TextStyle::new(Color::WHITE, u32::MAX).with_weight(GlyphWeight::Regular),
    );
    assert_eq!(fb.get_pixel(0, 0), Color::GREEN);
}

#[test]
fn translucent_border_does_not_blend_corners_twice() {
    let mut fb = Framebuffer::new(3, 3).expect("small surface");
    let color = Color::rgba(200, 10, 20, 128);
    draw_rect_outline(
        &mut fb,
        Rect::new(0, 0, 3, 3),
        1,
        CornerRadius::SQUARE,
        color,
    );
    assert_eq!(fb.get_pixel(0, 0), color);
    assert_eq!(fb.get_pixel(1, 1), Color::TRANSPARENT);
    assert_eq!(fb.get_pixel(2, 2), color);
}

#[test]
fn glyph_pixels_and_spaces_follow_bitmap_cells() {
    let mut fb = Framebuffer::new(24, 16).expect("text surface");
    draw_text(
        &mut fb,
        0,
        0,
        "A B",
        TextStyle::new(Color::RED, 1).with_weight(GlyphWeight::Regular),
    );
    assert_eq!(fb.get_pixel(2, 2), Color::RED);
    assert_eq!(fb.get_pixel(0, 0), Color::TRANSPARENT);
    assert_eq!(fb.get_pixel(10, 2), Color::TRANSPARENT);
    assert_eq!(fb.get_pixel(18, 2), Color::RED);
}

#[test]
fn fractional_text_scale_changes_pixel_extent_deterministically() {
    let mut one = Framebuffer::new(32, 20).expect("one-scale surface");
    draw_text(&mut one, 0, 0, "A", TextStyle::new(Color::RED, 1));
    let mut fractional = Framebuffer::new(32, 20).expect("fractional surface");
    draw_text(
        &mut fractional,
        0,
        0,
        "A",
        TextStyle::new(Color::RED, 1)
            .with_display_scale(DisplayScale::from_milli(1_500).expect("150 percent")),
    );
    let one_pixels = one.pixels().iter().filter(|pixel| **pixel != 0).count();
    let scaled_pixels = fractional
        .pixels()
        .iter()
        .filter(|pixel| **pixel != 0)
        .count();
    assert!(scaled_pixels > one_pixels);
    assert_eq!(one.get_pixel(2, 2), Color::RED);
}

#[test]
fn extreme_fractional_text_scale_clips_without_panicking() {
    let mut framebuffer = Framebuffer::new(8, 8).expect("surface");
    draw_text(
        &mut framebuffer,
        0,
        0,
        "A",
        TextStyle::new(Color::RED, u32::MAX)
            .with_display_scale(DisplayScale::from_milli(u32::MAX).expect("validated scale")),
    );
    assert!(framebuffer.pixels().iter().all(|pixel| *pixel == 0));
}

#[test]
fn bold_text_paints_more_pixels_without_moving_the_run() {
    let ink = |weight| {
        let mut fb = Framebuffer::new(96, 20).expect("text surface");
        draw_text(
            &mut fb,
            0,
            0,
            "Backend",
            TextStyle::new(Color::BLACK, 1).with_weight(weight),
        );
        fb.pixels().iter().filter(|pixel| **pixel != 0).count()
    };
    let regular = ink(GlyphWeight::Regular);
    let bold = ink(GlyphWeight::Bold);
    assert!(bold > regular, "bold painted {bold}, regular {regular}");

    // The advance is unchanged, so the trailing column of the last cell is
    // the same in both weights and no glyph spills past the run.
    let mut wide = Framebuffer::new(96, 20).expect("text surface");
    draw_text(
        &mut wide,
        0,
        0,
        "Backend",
        TextStyle::new(Color::BLACK, 1).with_weight(GlyphWeight::Bold),
    );
    for y in 0..20 {
        assert_eq!(
            wide.get_pixel(56, y),
            Color::TRANSPARENT,
            "bold run reached past its seven cells at row {y}"
        );
    }
}

#[test]
fn line_clips_extreme_endpoints_before_traversal() {
    let mut fb = Framebuffer::new(4, 3).expect("line surface");
    draw_line(
        &mut fb,
        (i32::MIN, i32::MIN),
        (i32::MAX, i32::MAX),
        Color::BLUE,
    );
    assert_eq!(fb.get_pixel(0, 0), Color::BLUE);
    assert_eq!(fb.get_pixel(1, 1), Color::BLUE);
    assert_eq!(fb.get_pixel(2, 2), Color::BLUE);
    assert_eq!(fb.get_pixel(3, 0), Color::TRANSPARENT);
}

#[test]
fn translucent_line_composites_each_pixel_once() {
    let mut fb = Framebuffer::new(3, 1).expect("line surface");
    fb.clear(Color::WHITE);
    draw_line(&mut fb, (-4, 0), (8, 0), Color::rgba(0, 0, 255, 128));
    assert_eq!(fb.get_pixel(0, 0), Color::rgba(127, 127, 255, 255));
    assert_eq!(fb.get_pixel(1, 0), Color::rgba(127, 127, 255, 255));
    assert_eq!(fb.get_pixel(2, 0), Color::rgba(127, 127, 255, 255));
}
