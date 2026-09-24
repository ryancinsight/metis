//! Clipped rendering reproduces unclipped rendering inside the clip and
//! leaves every other pixel alone.

use super::*;
use crate::typeface::{GlyphWeight, TextSize, TextStyle, draw_text};

/// Deterministic xorshift source so a failing scene replays exactly.
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
        low + i32::try_from(self.next() % span).expect("bounded span fits i32")
    }

    fn color(&mut self) -> Color {
        let bits = self.next();
        let channel = |shift: u32| u8::try_from((bits >> shift) & 0xff).expect("one byte");
        Color::rgba(channel(0), channel(8), channel(16), channel(24).max(24))
    }

    fn rect(&mut self) -> Rect {
        Rect::new(
            self.in_range(-20, 90),
            self.in_range(-20, 70),
            self.in_range(1, 60),
            self.in_range(1, 50),
        )
    }
}

const WIDTH: u32 = 96;
const HEIGHT: u32 = 72;

/// Draws one pseudo-random scene covering every primitive.
fn scene(fb: &mut Framebuffer, seed: u64) {
    let mut sequence = Sequence(seed);
    for _ in 0..3 {
        let rect = sequence.rect();
        let radius = CornerRadius::clamped(sequence.in_range(0, 14), rect);
        let blur = u32::try_from(sequence.in_range(0, 16)).expect("nonnegative blur");
        let shadow = BoxShadow::new(
            sequence.in_range(-6, 6),
            sequence.in_range(-6, 6),
            blur,
            sequence.color(),
        )
        .expect("blur within range");
        draw_box_shadow(fb, rect, radius, shadow);
        fill_rect(fb, rect, radius, sequence.color());
        let stops = [
            GradientStop {
                color: sequence.color(),
                position: None,
            },
            GradientStop {
                color: sequence.color(),
                position: None,
            },
        ];
        let degrees = f64::from(sequence.in_range(0, 359));
        let gradient = LinearGradient::new(degrees, &stops).expect("two finite stops");
        fill_gradient(fb, sequence.rect(), radius, &gradient);
        draw_rect_outline(
            fb,
            sequence.rect(),
            sequence.in_range(1, 4),
            radius,
            sequence.color(),
        );
        draw_line(
            fb,
            (sequence.in_range(-30, 120), sequence.in_range(-30, 100)),
            (sequence.in_range(-30, 120), sequence.in_range(-30, 100)),
            sequence.color(),
        );
        let points = [
            (sequence.in_range(-10, 100), sequence.in_range(-10, 80)),
            (sequence.in_range(-10, 100), sequence.in_range(-10, 80)),
            (sequence.in_range(-10, 100), sequence.in_range(-10, 80)),
        ];
        let width = StrokeWidth::new(u32::try_from(sequence.in_range(1, 7)).expect("positive"))
            .expect("width within range");
        draw_polyline(
            fb,
            &points,
            width,
            LineCap::Round,
            LineJoin::Miter,
            sequence.color(),
        );
        let size = TextSize::new(f64::from(sequence.in_range(9, 30))).expect("size within range");
        let style = TextStyle::new(sequence.color(), size).with_weight(GlyphWeight::Bold);
        draw_text(
            fb,
            sequence.in_range(-20, 60),
            sequence.in_range(-20, 60),
            "Clip Wg",
            style,
        );
    }
}

#[test]
fn clipped_scenes_match_unclipped_inside_and_leave_the_rest() {
    let backdrop = Color::rgba(240, 244, 248, 255);
    for case in 0..96_u64 {
        let seed = 0x9e37_79b9_7f4a_7c15 ^ (case * 0x2545_f491);
        let mut full = Framebuffer::new(WIDTH, HEIGHT).expect("reference surface");
        full.clear(backdrop);
        scene(&mut full, seed);

        let mut regions = Sequence(seed.rotate_left(17) | 1);
        let region = Rect::new(
            regions.in_range(-10, 90),
            regions.in_range(-10, 66),
            regions.in_range(0, 50),
            regions.in_range(0, 40),
        );
        let mut clipped = Framebuffer::new(WIDTH, HEIGHT).expect("clipped surface");
        let untouched = Color::rgba(1, 2, 3, 4);
        clipped.clear(untouched);
        clipped.render_clipped(region, |fb| {
            fb.clear(backdrop);
            scene(fb, seed);
        });
        assert_eq!(
            clipped.clip(),
            full.clip(),
            "case {case}: the clip was restored"
        );
        for y in 0..i32::try_from(HEIGHT).expect("height fits") {
            for x in 0..i32::try_from(WIDTH).expect("width fits") {
                let expected = if region.contains(x, y) {
                    full.get_pixel(x, y)
                } else {
                    untouched
                };
                assert_eq!(
                    clipped.get_pixel(x, y),
                    expected,
                    "case {case} pixel ({x}, {y})"
                );
            }
        }
    }
}

#[test]
fn a_panicking_draw_restores_the_whole_surface_clip() {
    let mut fb = Framebuffer::new(8, 8).expect("surface");
    let whole = fb.clip();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        fb.render_clipped(Rect::new(2, 2, 3, 3), |_| panic!("draw failed"));
    }));
    assert!(outcome.is_err(), "the draw panicked");
    assert_eq!(fb.clip(), whole);
    fb.set_pixel(0, 0, Color::WHITE);
    assert_eq!(
        fb.get_pixel(0, 0),
        Color::WHITE,
        "writes outside the old clip land again"
    );
}

/// Asserts every pixel `draw` changes on a blank surface lies in `extent`.
fn assert_within(extent: Rect, case: u64, draw: impl FnOnce(&mut Framebuffer)) {
    let mut fb = Framebuffer::new(WIDTH, HEIGHT).expect("probe surface");
    let blank = Color::rgba(240, 244, 248, 255);
    fb.clear(blank);
    draw(&mut fb);
    for y in 0..i32::try_from(HEIGHT).expect("height fits") {
        for x in 0..i32::try_from(WIDTH).expect("width fits") {
            if fb.get_pixel(x, y) != blank {
                assert!(
                    extent.contains(x, y),
                    "case {case}: ({x}, {y}) painted outside {extent:?}"
                );
            }
        }
    }
}

#[test]
fn reported_extents_hold_every_painted_pixel() {
    let mut sequence = Sequence(0x51_7cc1_b727_220a);
    for case in 0..256_u64 {
        let rect = sequence.rect();
        let radius = CornerRadius::clamped(sequence.in_range(0, 14), rect);
        let shadow = BoxShadow::new(
            sequence.in_range(-8, 8),
            sequence.in_range(-8, 8),
            u32::try_from(sequence.in_range(0, 24)).expect("nonnegative blur"),
            Color::rgba(0, 0, 0, 255),
        )
        .expect("blur within range");
        assert_within(shadow.extent(rect), case, |fb| {
            draw_box_shadow(fb, rect, radius, shadow);
        });

        let points = [
            (sequence.in_range(-10, 100), sequence.in_range(-10, 80)),
            (sequence.in_range(-10, 100), sequence.in_range(-10, 80)),
            (sequence.in_range(-10, 100), sequence.in_range(-10, 80)),
        ];
        let width = StrokeWidth::new(u32::try_from(sequence.in_range(1, 9)).expect("positive"))
            .expect("width within range");
        for join in [LineJoin::Miter, LineJoin::Bevel, LineJoin::Round] {
            let extent = polyline_extent(&points, width, join).expect("three points");
            assert_within(extent, case, |fb| {
                draw_polyline(fb, &points, width, LineCap::Square, join, Color::BLACK);
            });
        }

        let size = TextSize::new(f64::from(sequence.in_range(6, 48))).expect("size within range");
        for weight in [GlyphWeight::Regular, GlyphWeight::Bold] {
            let style = TextStyle::new(Color::BLACK, size).with_weight(weight);
            let (x, y) = (sequence.in_range(-20, 60), sequence.in_range(-30, 50));
            let text = r"gjpqy ÅÉÎ|/\_{}@Wm";
            let extent = style.extent(x, y, text).expect("a nonempty run");
            assert_within(extent, case, |fb| draw_text(fb, x, y, text, style));
        }
    }
    assert_eq!(
        TextStyle::new(Color::BLACK, TextSize::new(12.0).expect("size")).extent(0, 0, "\n"),
        None
    );
}
