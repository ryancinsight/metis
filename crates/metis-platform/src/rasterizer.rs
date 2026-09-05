//! Clipped rectangle and bitmap text drawing, bounded by framebuffer area.

use crate::font::{FONT_WIDTH, draw_glyph};
use crate::framebuffer::{Color, Framebuffer, Rect};

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
}
