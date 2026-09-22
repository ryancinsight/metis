//! Clipped rectangle, line and bitmap text drawing, bounded by framebuffer area.

use crate::font::{FONT_WIDTH, TextStyle, draw_glyph_cells};
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
pub fn draw_text(fb: &mut Framebuffer, x: i32, y: i32, text: &str, style: TextStyle) {
    let effective_milli = u64::from(style.scale.max(1)) * u64::from(style.display_scale.milli());
    // Bold thickens strokes inside the cell, so the advance is the same in
    // either weight and a weight change never reflows a run.
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
            draw_glyph_cells(fb, origin, y, c, style.color, effective_milli, style.weight);
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
mod tests;
