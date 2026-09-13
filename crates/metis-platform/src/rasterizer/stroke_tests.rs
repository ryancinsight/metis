//! Value-semantic tests for bounded polyline strokes.

use super::*;

#[test]
fn stroke_width_is_positive_and_caps_have_distinct_extent() {
    assert!(StrokeWidth::new(0).is_err());
    let width = StrokeWidth::new(3).expect("positive stroke width");
    let mut butt = Framebuffer::new(8, 5).expect("stroke surface");
    draw_polyline(
        &mut butt,
        &[(2, 2), (4, 2)],
        width,
        LineCap::Butt,
        LineJoin::Bevel,
        Color::RED,
    );
    assert_eq!(butt.get_pixel(1, 2), Color::TRANSPARENT);
    assert_eq!(butt.get_pixel(2, 1), Color::RED);
    let mut square = Framebuffer::new(8, 5).expect("stroke surface");
    draw_polyline(
        &mut square,
        &[(2, 2), (4, 2)],
        width,
        LineCap::Square,
        LineJoin::Bevel,
        Color::RED,
    );
    assert_eq!(square.get_pixel(1, 2), Color::RED);
    assert_eq!(square.get_pixel(0, 2), Color::TRANSPARENT);
}

#[test]
fn overlapping_polyline_segments_blend_each_pixel_once() {
    let mut framebuffer = Framebuffer::new(5, 5).expect("stroke surface");
    framebuffer.clear(Color::WHITE);
    let color = Color::rgba(0, 0, 255, 128);
    draw_polyline(
        &mut framebuffer,
        &[(1, 2), (3, 2), (3, 4)],
        StrokeWidth::new(1).expect("positive stroke width"),
        LineCap::Butt,
        LineJoin::Bevel,
        color,
    );
    assert_eq!(framebuffer.get_pixel(3, 2), Color::rgba(127, 127, 255, 255));
}

#[test]
fn join_styles_control_the_outer_corner() {
    let width = StrokeWidth::new(4).expect("positive stroke width");
    let render = |join| {
        let mut framebuffer = Framebuffer::new(16, 12).expect("stroke surface");
        draw_polyline(
            &mut framebuffer,
            &[(2, 8), (6, 2), (10, 8)],
            width,
            LineCap::Butt,
            join,
            Color::RED,
        );
        framebuffer
    };
    let miter = render(LineJoin::Miter);
    let bevel = render(LineJoin::Bevel);
    let round = render(LineJoin::Round);
    assert_eq!(miter.get_pixel(5, 0), Color::RED);
    assert_eq!(bevel.get_pixel(5, 0), Color::TRANSPARENT);
    assert_eq!(round.get_pixel(6, 0), Color::RED);
    assert_eq!(round.get_pixel(5, 0), Color::TRANSPARENT);
}

#[test]
fn extreme_coordinates_and_width_stay_inside_the_surface_bound() {
    let mut framebuffer = Framebuffer::new(2, 2).expect("stroke surface");
    draw_polyline(
        &mut framebuffer,
        &[(i32::MIN, i32::MIN), (i32::MAX, i32::MAX)],
        StrokeWidth::new(u32::MAX).expect("positive stroke width"),
        LineCap::Square,
        LineJoin::Miter,
        Color::GREEN,
    );
    assert_eq!(framebuffer.pixels(), &[0xff38_a169; 4]);
}
