//! Value-semantic tests for antialiased rounded rectangles.

use super::*;
use crate::rasterizer::{draw_rect_outline, fill_bounds, fill_rect};

/// Counts pixels that are neither untouched nor fully covered.
fn partial_pixels(fb: &Framebuffer, background: Color, foreground: Color) -> usize {
    fb.pixels()
        .iter()
        .filter(|packed| {
            let color = unpack(**packed);
            color != background && color != foreground
        })
        .count()
}

fn unpack(packed: u32) -> Color {
    let [a, r, g, b] = packed.to_be_bytes();
    Color::rgba(r, g, b, a)
}

fn surface(width: u32, height: u32, background: Color) -> Framebuffer {
    let mut fb = Framebuffer::new(width, height).expect("test surface");
    fb.clear(background);
    fb
}

#[test]
fn clamped_radius_never_exceeds_half_the_shorter_side() {
    let rect = Rect::new(0, 0, 40, 24);
    assert_eq!(CornerRadius::clamped(8, rect).pixels(), 8);
    assert_eq!(CornerRadius::clamped(100, rect).pixels(), 12);
    assert_eq!(CornerRadius::clamped(12, rect).pixels(), 12);
    for requested in [-5, 0] {
        assert!(CornerRadius::clamped(requested, rect).is_square());
    }
    for empty in [Rect::new(0, 0, 0, 10), Rect::new(0, 0, 10, 0)] {
        assert!(CornerRadius::clamped(4, empty).is_square());
    }
}

#[test]
fn square_radius_reproduces_the_unrounded_fill_exactly() {
    for rect in [
        Rect::new(2, 3, 20, 14),
        Rect::new(-4, -2, 30, 25),
        Rect::new(0, 0, 32, 20),
    ] {
        for color in [Color::BLUE, Color::rgba(200, 40, 90, 137)] {
            let mut rounded = surface(32, 20, Color::WHITE);
            let mut square = surface(32, 20, Color::WHITE);
            fill_rect(&mut rounded, rect, CornerRadius::SQUARE, color);
            fill_bounds(
                &mut square,
                i64::from(rect.x),
                i64::from(rect.y),
                i64::from(rect.x) + i64::from(rect.width),
                i64::from(rect.y) + i64::from(rect.height),
                color,
            );
            assert_eq!(
                rounded.pixels(),
                square.pixels(),
                "a square radius changed {rect:?}"
            );
        }
    }
}

#[test]
fn rounded_corners_clear_the_corner_and_keep_the_centre() {
    let rect = Rect::new(0, 0, 40, 40);
    let radius = CornerRadius::clamped(12, rect);
    let mut fb = surface(40, 40, Color::WHITE);
    fill_rect(&mut fb, rect, radius, Color::BLUE);
    // The extreme corner pixel lies outside the arc and keeps the background.
    for (x, y) in [(0, 0), (39, 0), (0, 39), (39, 39)] {
        assert_eq!(
            fb.get_pixel(x, y),
            Color::WHITE,
            "corner ({x}, {y}) was painted"
        );
    }
    // The centre, the edge midpoints and the straight edges are fully covered.
    for (x, y) in [(20, 20), (20, 0), (0, 20), (39, 20), (20, 39)] {
        assert_eq!(
            fb.get_pixel(x, y),
            Color::BLUE,
            "({x}, {y}) is not fully covered"
        );
    }
}

#[test]
fn corner_coverage_is_antialiased_and_symmetric() {
    let rect = Rect::new(0, 0, 40, 40);
    let radius = CornerRadius::clamped(12, rect);
    let mut fb = surface(40, 40, Color::WHITE);
    fill_rect(&mut fb, rect, radius, Color::BLUE);
    // An unantialiased arc produces only background or foreground pixels.
    let partial = partial_pixels(&fb, Color::WHITE, Color::BLUE);
    assert!(
        partial >= 16,
        "only {partial} partially covered pixels; the arc is not antialiased"
    );
    // The shape is symmetric, so mirrored pixels carry equal coverage.
    for y in 0..40 {
        for x in 0..40 {
            assert_eq!(
                fb.get_pixel(x, y),
                fb.get_pixel(39 - x, y),
                "({x}, {y}) is not mirrored horizontally"
            );
            assert_eq!(
                fb.get_pixel(x, y),
                fb.get_pixel(x, 39 - y),
                "({x}, {y}) is not mirrored vertically"
            );
        }
    }
}

#[test]
fn rounded_border_leaves_the_interior_untouched() {
    let rect = Rect::new(0, 0, 40, 30);
    let radius = CornerRadius::clamped(10, rect);
    let mut fb = surface(40, 30, Color::WHITE);
    draw_rect_outline(&mut fb, rect, 3, radius, Color::RED);
    // Inside the ring the background survives; on the straight edge it does not.
    assert_eq!(fb.get_pixel(20, 15), Color::WHITE);
    assert_eq!(fb.get_pixel(20, 0), Color::RED);
    assert_eq!(fb.get_pixel(20, 2), Color::RED);
    assert_eq!(fb.get_pixel(20, 4), Color::WHITE);
    assert_eq!(fb.get_pixel(0, 15), Color::RED);
    // The extreme corner is outside the outer arc.
    assert_eq!(fb.get_pixel(0, 0), Color::WHITE);
}

#[test]
fn a_radius_beyond_the_shape_and_an_empty_rectangle_paint_nothing_invalid() {
    let mut fb = surface(20, 20, Color::WHITE);
    // A fully clamped radius produces a capsule, never an inverted shape.
    let rect = Rect::new(2, 2, 16, 16);
    fill_rect(&mut fb, rect, CornerRadius::clamped(999, rect), Color::BLUE);
    assert_eq!(fb.get_pixel(10, 10), Color::BLUE);
    assert_eq!(fb.get_pixel(2, 2), Color::WHITE);

    for empty in [Rect::new(0, 0, 0, 8), Rect::new(0, 0, 8, -3)] {
        let mut untouched = surface(20, 20, Color::WHITE);
        fill_rect(
            &mut untouched,
            empty,
            CornerRadius::clamped(4, empty),
            Color::RED,
        );
        draw_rect_outline(
            &mut untouched,
            empty,
            2,
            CornerRadius::clamped(4, empty),
            Color::RED,
        );
        assert!(
            untouched
                .pixels()
                .iter()
                .all(|p| unpack(*p) == Color::WHITE),
            "an empty rectangle painted pixels"
        );
    }
}

#[test]
fn off_surface_rounded_geometry_clips_without_panicking() {
    let mut fb = surface(16, 16, Color::WHITE);
    for rect in [
        Rect::new(-20, -20, 60, 60),
        Rect::new(i32::MAX - 4, 0, 40, 40),
        Rect::new(-1, -1, i32::MAX, i32::MAX),
    ] {
        let radius = CornerRadius::clamped(9, rect);
        fill_rect(&mut fb, rect, radius, Color::rgba(10, 20, 30, 128));
        draw_rect_outline(&mut fb, rect, 2, radius, Color::rgba(10, 20, 30, 128));
    }
}

#[test]
fn a_transparent_source_leaves_a_rounded_shape_untouched() {
    let rect = Rect::new(1, 1, 18, 18);
    let radius = CornerRadius::clamped(6, rect);
    let mut fb = surface(20, 20, Color::WHITE);
    fill_rect(&mut fb, rect, radius, Color::TRANSPARENT);
    draw_rect_outline(&mut fb, rect, 2, radius, Color::TRANSPARENT);
    assert!(fb.pixels().iter().all(|p| unpack(*p) == Color::WHITE));
}
