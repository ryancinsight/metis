//! Clipped rectangle, line and bitmap text drawing, bounded by framebuffer area.

use crate::DisplayScale;
use crate::font::{FONT_WIDTH, draw_glyph_scaled};
use crate::framebuffer::{Color, Framebuffer, Rect, SourceOver};
mod round_rect;
mod stroke;

const LEFT: u8 = 1;
const RIGHT: u8 = 2;
const TOP: u8 = 4;
const BOTTOM: u8 = 8;

pub use round_rect::CornerRadius;
use round_rect::{RoundRect, composite_shape};
pub use stroke::{LineCap, LineJoin, MAX_STROKE_POINTS, StrokeWidth, draw_polyline};

/// Composites a color over the visible part of an axis-aligned span rectangle.
///
/// The bounds are clipped once, then each covered row is written as one
/// contiguous span. An opaque source replaces the span outright because
/// source-over with a fully opaque source reduces to the source value; a
/// translucent source composites through the shared [`SourceOver`] terms. The
/// composited result is identical to blending each pixel on its own.
pub(crate) fn fill_bounds(
    fb: &mut Framebuffer,
    left: i64,
    top: i64,
    right: i64,
    bottom: i64,
    color: Color,
) {
    let source = SourceOver::new(color);
    if source.is_transparent() {
        return;
    }
    let clamp = |value: i64, limit: u32| -> u32 {
        u32::try_from(value.clamp(0, i64::from(limit)))
            .expect("invariant: a clamped bound is a nonnegative surface coordinate")
    };
    let left = clamp(left, fb.width());
    let right = clamp(right, fb.width());
    let top = clamp(top, fb.height());
    let bottom = clamp(bottom, fb.height());
    if right <= left || bottom <= top {
        return;
    }
    if source.is_opaque() {
        let packed = source.packed();
        for y in top..bottom {
            fb.row_span_mut(y, left, right).fill(packed);
        }
    } else {
        for y in top..bottom {
            fb.composite_span(y, left, right, source);
        }
    }
}

/// Fills the visible intersection of an axis-aligned rectangle.
///
/// [`CornerRadius::SQUARE`] takes the unrounded span path, so square output is
/// unchanged. A rounded radius antialiases the corner arcs by coverage and
/// leaves the straight edges exact.
pub fn fill_rect(fb: &mut Framebuffer, rect: Rect, radius: CornerRadius, color: Color) {
    if radius.is_square() {
        fill_bounds(
            fb,
            i64::from(rect.x),
            i64::from(rect.y),
            i64::from(rect.x) + i64::from(rect.width),
            i64::from(rect.y) + i64::from(rect.height),
            color,
        );
        return;
    }
    let Some(outer) = RoundRect::new(rect, radius) else {
        return;
    };
    composite_shape(fb, outer, None, color);
}

/// Draws an inward border, blending each covered pixel exactly once.
///
/// A rounded radius paints the ring between the outer shape and the shape
/// inset by the border width, so the border follows the same arc as the fill
/// it encloses.
pub fn draw_rect_outline(
    fb: &mut Framebuffer,
    rect: Rect,
    width: i32,
    radius: CornerRadius,
    color: Color,
) {
    if width <= 0 || rect.width <= 0 || rect.height <= 0 {
        return;
    }
    if !radius.is_square() {
        let Some(outer) = RoundRect::new(rect, radius) else {
            return;
        };
        composite_shape(fb, outer, outer.inset(f64::from(width)), color);
        return;
    }
    let left = i64::from(rect.x);
    let top = i64::from(rect.y);
    let right = left + i64::from(rect.width);
    let bottom = top + i64::from(rect.height);
    let width = i64::from(width);
    let inner_left = (left + width).min(right);
    let inner_right = (right - width).max(inner_left);
    let inner_top = (top + width).min(bottom);
    let inner_bottom = (bottom - width).max(inner_top);
    fill_bounds(fb, left, top, right, inner_top, color);
    fill_bounds(fb, left, inner_bottom, right, bottom, color);
    fill_bounds(fb, left, inner_top, inner_left, inner_bottom, color);
    fill_bounds(fb, inner_right, inner_top, right, inner_bottom, color);
}

/// Draws a clipped one-pixel line with source-over compositing.
///
/// Endpoints may be outside the framebuffer. Integer clipping happens before
/// Bresenham traversal, so an adversarial segment cannot force work proportional
/// to its off-screen length. The viewport is the inclusive pixel rectangle.
///
/// # Panics
///
/// Does not panic for a valid [`Framebuffer`]; the conversion checks encode the
/// clipping invariant that every traversed coordinate fits the framebuffer's
/// signed coordinate range.
pub fn draw_line(fb: &mut Framebuffer, start: (i32, i32), end: (i32, i32), color: Color) {
    let Some((x0, y0, x1, y1)) = clip_line(
        i64::from(start.0),
        i64::from(start.1),
        i64::from(end.0),
        i64::from(end.1),
        i64::from(fb.width()) - 1,
        i64::from(fb.height()) - 1,
    ) else {
        return;
    };

    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let step_x = if x0 < x1 { 1 } else { -1 };
    let step_y = if y0 < y1 { 1 } else { -1 };
    let mut error = dx + dy;
    let (mut x, mut y) = (x0, y0);
    loop {
        fb.blend_pixel(
            i32::try_from(x).expect("invariant: clipped x coordinate fits i32"),
            i32::try_from(y).expect("invariant: clipped y coordinate fits i32"),
            color,
        );
        if x == x1 && y == y1 {
            break;
        }
        let doubled = error * 2;
        if doubled >= dy {
            error += dy;
            x += step_x;
        }
        if doubled <= dx {
            error += dx;
            y += step_y;
        }
    }
}

fn clip_line(
    mut x0: i64,
    mut y0: i64,
    mut x1: i64,
    mut y1: i64,
    max_x: i64,
    max_y: i64,
) -> Option<(i64, i64, i64, i64)> {
    let mut first = out_code(x0, y0, max_x, max_y);
    let mut second = out_code(x1, y1, max_x, max_y);
    for _ in 0..4 {
        if first | second == 0 {
            return Some((x0, y0, x1, y1));
        }
        if first & second != 0 {
            return None;
        }
        let outside = if first != 0 { first } else { second };
        let (x, y) = if outside & TOP != 0 {
            let y = 0;
            (interpolate(y0, x0, y1, x1, y), y)
        } else if outside & BOTTOM != 0 {
            let y = max_y;
            (interpolate(y0, x0, y1, x1, y), y)
        } else if outside & RIGHT != 0 {
            let x = max_x;
            (x, interpolate(x0, y0, x1, y1, x))
        } else {
            let x = 0;
            (x, interpolate(x0, y0, x1, y1, x))
        };
        let x = x.clamp(0, max_x);
        let y = y.clamp(0, max_y);
        if outside == first {
            x0 = x;
            y0 = y;
            first = out_code(x0, y0, max_x, max_y);
        } else {
            x1 = x;
            y1 = y;
            second = out_code(x1, y1, max_x, max_y);
        }
    }
    None
}

fn out_code(x: i64, y: i64, max_x: i64, max_y: i64) -> u8 {
    (if x < 0 {
        LEFT
    } else if x > max_x {
        RIGHT
    } else {
        0
    }) | (if y < 0 {
        TOP
    } else if y > max_y {
        BOTTOM
    } else {
        0
    })
}

fn interpolate(first_a: i64, first_b: i64, second_a: i64, second_b: i64, target: i64) -> i64 {
    let numerator =
        (i128::from(second_b) - i128::from(first_b)) * (i128::from(target) - i128::from(first_a));
    let denominator = i128::from(second_a) - i128::from(first_a);
    let value = i128::from(first_b) + numerator / denominator;
    i64::try_from(value).expect("invariant: line interpolation remains in coordinate range")
}

/// Renders one horizontal text run; newline characters have no advance.
///
/// Scale zero means one. Unsupported characters use the font replacement glyph.
pub fn draw_text(fb: &mut Framebuffer, x: i32, y: i32, text: &str, color: Color, scale: u32) {
    draw_text_scaled(fb, x, y, text, color, scale, DisplayScale::ONE);
}

/// Renders one horizontal text run at a fractional device scale.
///
/// The integer `scale` remains the authored bitmap multiplier. The validated
/// display scale maps authored pixels to physical pixels with deterministic
/// fixed-point rounding, so native DPI changes repaint geometry and text from
/// the same display list.
pub fn draw_text_scaled(
    fb: &mut Framebuffer,
    x: i32,
    y: i32,
    text: &str,
    color: Color,
    scale: u32,
    display_scale: DisplayScale,
) {
    let effective_milli = u64::from(scale.max(1)) * u64::from(display_scale.milli());
    let advance = scaled_extent(u64::from(FONT_WIDTH), effective_milli);
    let mut cursor = i64::from(x);
    for c in text.chars().filter(|c| *c != '\n') {
        if cursor >= i64::from(fb.width()) {
            break;
        }
        let Ok(origin) = i32::try_from(cursor) else {
            break;
        };
        if cursor.saturating_add(advance) > 0 {
            draw_glyph_scaled(fb, origin, y, c, color, scale, display_scale);
        }
        cursor = cursor.saturating_add(advance);
    }
}

fn scaled_extent(value: u64, effective_milli: u64) -> i64 {
    let rounded = (u128::from(value) * u128::from(effective_milli) + 500) / 1_000;
    i64::try_from(rounded.min(u128::from(u64::MAX / 2)))
        .expect("invariant: clipped text extent fits i64")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::draw_glyph;

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
            let channel = |shift: u32| {
                u8::try_from((bits >> shift) & 0xff).expect("byte extracted from a word")
            };
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
                draw_glyph(&mut runs, 3, 5, character, color, requested);
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
        draw_text(&mut fb, i32::MAX, i32::MIN, "A", Color::WHITE, u32::MAX);
        assert_eq!(fb.get_pixel(0, 0), Color::GREEN);
        draw_glyph(&mut fb, 0, 0, 'A', Color::WHITE, u32::MAX);
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
        draw_text(&mut fb, 0, 0, "A B", Color::RED, 1);
        assert_eq!(fb.get_pixel(2, 2), Color::RED);
        assert_eq!(fb.get_pixel(0, 0), Color::TRANSPARENT);
        assert_eq!(fb.get_pixel(10, 2), Color::TRANSPARENT);
        assert_eq!(fb.get_pixel(18, 2), Color::RED);
    }

    #[test]
    fn fractional_text_scale_changes_pixel_extent_deterministically() {
        let mut one = Framebuffer::new(32, 20).expect("one-scale surface");
        draw_text_scaled(&mut one, 0, 0, "A", Color::RED, 1, DisplayScale::ONE);
        let mut fractional = Framebuffer::new(32, 20).expect("fractional surface");
        draw_text_scaled(
            &mut fractional,
            0,
            0,
            "A",
            Color::RED,
            1,
            DisplayScale::from_milli(1_500).expect("150 percent"),
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
        draw_text_scaled(
            &mut framebuffer,
            0,
            0,
            "A",
            Color::RED,
            u32::MAX,
            DisplayScale::from_milli(u32::MAX).expect("validated scale"),
        );
        assert!(framebuffer.pixels().iter().all(|pixel| *pixel == 0));
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
}
