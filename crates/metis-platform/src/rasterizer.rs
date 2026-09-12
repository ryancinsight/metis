//! Clipped rectangle and bitmap text drawing, bounded by framebuffer area.

use crate::font::{FONT_WIDTH, draw_glyph};
use crate::framebuffer::{Color, Framebuffer, Rect};

const LEFT: u8 = 1;
const RIGHT: u8 = 2;
const TOP: u8 = 4;
const BOTTOM: u8 = 8;

pub(crate) fn fill_bounds(
    fb: &mut Framebuffer,
    left: i64,
    top: i64,
    right: i64,
    bottom: i64,
    color: Color,
) {
    let left = left.clamp(0, i64::from(fb.width()));
    let top = top.clamp(0, i64::from(fb.height()));
    let right = right.clamp(0, i64::from(fb.width()));
    let bottom = bottom.clamp(0, i64::from(fb.height()));
    for y in top..bottom {
        for x in left..right {
            fb.blend_pixel(
                i32::try_from(x).expect("invariant: framebuffer coordinates fit i32"),
                i32::try_from(y).expect("invariant: framebuffer coordinates fit i32"),
                color,
            );
        }
    }
}

/// Fills the visible intersection of an axis-aligned rectangle.
pub fn fill_rect(fb: &mut Framebuffer, rect: Rect, color: Color) {
    fill_bounds(
        fb,
        i64::from(rect.x),
        i64::from(rect.y),
        i64::from(rect.x) + i64::from(rect.width),
        i64::from(rect.y) + i64::from(rect.height),
        color,
    );
}

/// Draws an inward border, blending each corner pixel exactly once.
pub fn draw_rect_outline(fb: &mut Framebuffer, rect: Rect, width: i32, color: Color) {
    if width <= 0 || rect.width <= 0 || rect.height <= 0 {
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
    let advance = i64::from(FONT_WIDTH) * i64::from(scale.max(1));
    let mut cursor = i64::from(x);
    for c in text.chars().filter(|c| *c != '\n') {
        if cursor >= i64::from(fb.width()) {
            break;
        }
        let Ok(origin) = i32::try_from(cursor) else {
            break;
        };
        if cursor + advance > 0 {
            draw_glyph(fb, origin, y, c, color, scale);
        }
        cursor += advance;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extreme_geometry_clips_without_overflow() {
        let mut fb = Framebuffer::new(2, 2).expect("small surface");
        fill_rect(&mut fb, Rect::new(-1, -1, i32::MAX, i32::MAX), Color::GREEN);
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
        draw_rect_outline(&mut fb, Rect::new(0, 0, 3, 3), 1, color);
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
