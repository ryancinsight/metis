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

/// Composites the shape one pixel at a time from the coverage definition.
///
/// This is the contract `composite_shape` optimizes: coverage is the area
/// inside `outer` and outside `inner`, integrated over the subsample rows,
/// and every pixel is composited exactly once.
fn reference_shape(fb: &mut Framebuffer, outer: RoundRect, inner: Option<RoundRect>, color: Color) {
    for row in 0..fb.height() {
        for column in 0..fb.width() {
            let mut total = 0.0;
            for index in 0..SUBSAMPLES {
                let step = u32::try_from(index).expect("the subsample count fits u32");
                let y = (f64::from(step) + 0.5).mul_add(SUBSAMPLE_RECIPROCAL, f64::from(row));
                let mut covered = overlap(outer.extent_at(y), f64::from(column));
                if let Some(inner) = inner {
                    covered -= overlap(inner.extent_at(y), f64::from(column));
                }
                total += covered.max(0.0);
            }
            composite_pixel(
                fb,
                row,
                f64::from(column),
                color,
                total * SUBSAMPLE_RECIPROCAL,
            );
        }
    }
}

#[test]
fn span_classification_matches_the_coverage_definition() {
    // Sizes and radii chosen so rows fall in every class: full spans, hollow
    // interiors, corner transitions, and rows the shape only partly spans.
    for (width, height, radius, border) in [
        (24_u32, 18_u32, 7, None),
        (24, 18, 7, Some(3)),
        (9, 9, 4, Some(1)),
        (32, 12, 6, Some(2)),
        (5, 21, 2, Some(4)),
        (16, 16, 8, Some(8)),
    ] {
        for color in [Color::rgba(200, 40, 90, 137), Color::BLUE] {
            for origin in [(0_i32, 0_i32), (-3, -5), (2, 1)] {
                let rect = Rect::new(
                    origin.0,
                    origin.1,
                    i32::try_from(width).expect("fixture width"),
                    i32::try_from(height).expect("fixture height"),
                );
                let Some(outer) = RoundRect::new(rect, CornerRadius::clamped(radius, rect)) else {
                    continue;
                };
                let inner = border.and_then(|width| outer.inset(f64::from(width)));
                let mut actual = surface(28, 24, Color::WHITE);
                let mut expected = surface(28, 24, Color::WHITE);
                composite_shape(&mut actual, outer, inner, &color);
                reference_shape(&mut expected, outer, inner, color);
                assert_eq!(
                    actual.pixels(),
                    expected.pixels(),
                    "{width}x{height} radius {radius} border {border:?} at {origin:?} diverged"
                );
            }
        }
    }
}

#[test]
fn a_translucent_rounded_border_composites_each_pixel_once() {
    // Compositing a pixel twice compounds opacity, which an opaque source
    // hides. The square path has the same guard.
    let rect = Rect::new(0, 0, 24, 20);
    let radius = CornerRadius::clamped(8, rect);
    let color = Color::rgba(200, 10, 20, 128);
    let mut once = surface(24, 20, Color::TRANSPARENT);
    draw_rect_outline(&mut once, rect, 3, radius, color);
    // On the straight edge the border is exactly the source at one pass.
    assert_eq!(once.get_pixel(12, 0), color);
    assert_eq!(once.get_pixel(12, 2), color);
    assert_eq!(once.get_pixel(0, 10), color);
    // No pixel anywhere exceeds a single composite of this source.
    for pixel in once.pixels() {
        let alpha = pixel.to_be_bytes()[0];
        assert!(
            alpha <= color.a,
            "a pixel reached alpha {alpha}, above one pass of {}",
            color.a
        );
    }
}

#[test]
fn off_surface_rounded_geometry_clips_to_the_visible_result() {
    let color = Color::rgba(10, 20, 30, 128);
    // A shape covering the whole surface leaves no background behind; its arcs
    // are off-screen, so every visible pixel is fully covered.
    let covering = Rect::new(-20, -20, 60, 60);
    let mut fb = surface(16, 16, Color::WHITE);
    fill_rect(&mut fb, covering, CornerRadius::clamped(9, covering), color);
    let mut reference = surface(16, 16, Color::WHITE);
    fill_rect(&mut reference, covering, CornerRadius::SQUARE, color);
    assert_eq!(
        fb.pixels(),
        reference.pixels(),
        "an off-screen arc changed the visible fill"
    );

    // Shapes entirely outside the surface, and extreme coordinates, paint
    // nothing and do not panic.
    for rect in [
        Rect::new(i32::MAX - 4, 0, 40, 40),
        Rect::new(-80, -80, 40, 40),
    ] {
        let mut untouched = surface(16, 16, Color::WHITE);
        let radius = CornerRadius::clamped(9, rect);
        fill_rect(&mut untouched, rect, radius, color);
        draw_rect_outline(&mut untouched, rect, 2, radius, color);
        assert!(
            untouched
                .pixels()
                .iter()
                .all(|p| unpack(*p) == Color::WHITE),
            "off-surface geometry painted {rect:?}"
        );
    }

    // An extent spanning the coordinate range clips rather than overflowing.
    // Its origin is one pixel off-screen, so the corner arc is visible and the
    // result is checked against the coverage definition rather than assumed
    // to be a full fill.
    let spanning = Rect::new(-1, -1, i32::MAX, i32::MAX);
    let spanning_radius = CornerRadius::clamped(9, spanning);
    let outer = RoundRect::new(spanning, spanning_radius).expect("a spanning shape has area");
    let mut spanned = surface(16, 16, Color::WHITE);
    let mut expected = surface(16, 16, Color::WHITE);
    fill_rect(&mut spanned, spanning, spanning_radius, color);
    reference_shape(&mut expected, outer, None, color);
    assert_eq!(
        spanned.pixels(),
        expected.pixels(),
        "a surface-spanning shape diverged from the coverage definition"
    );
    // The arc is genuinely on screen: the extreme corner stays background.
    assert_eq!(spanned.get_pixel(0, 0), Color::WHITE);
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
