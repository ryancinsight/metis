//! Value-semantic tests for linear gradients against CSS Images 3.

use super::*;
use crate::framebuffer::Rect;
use crate::rasterizer::{CornerRadius, fill_gradient, fill_rect};

/// Fractions are sums of about ten products of magnitude at most a few
/// units, each rounded once (unit roundoff 2^-53, about 1.1e-16), plus a
/// `sin_cos` within one ulp; 1e-12 leaves three orders of headroom.
const FRACTION_TOLERANCE: f64 = 1e-12;

fn stop(color: Color, position: Option<f64>) -> GradientStop {
    GradientStop { color, position }
}

fn gradient(degrees: f64, stops: &[GradientStop]) -> LinearGradient {
    LinearGradient::new(degrees, stops).expect("valid test gradient")
}

fn surface(width: u32, height: u32, background: Color) -> Framebuffer {
    let mut fb = Framebuffer::new(width, height).expect("test surface");
    fb.clear(background);
    fb
}

fn positions(gradient: &LinearGradient) -> Vec<f64> {
    gradient.stops().map(|stop| stop.position).collect()
}

/// Section 3.1 written out directly: the signed distance of a point from
/// the box center along the gradient direction, over the gradient length.
fn reference_fraction(degrees: f64, (width, height): (f64, f64), (x, y): (f64, f64)) -> f64 {
    let (sine, cosine) = degrees.to_radians().sin_cos();
    let length = (width * sine).abs() + (height * cosine).abs();
    0.5 + ((x - width / 2.0) * sine - (y - height / 2.0) * cosine) / length
}

#[test]
fn construction_rejects_stop_counts_and_nonfinite_values() {
    let two = [stop(Color::WHITE, None), stop(Color::BLACK, None)];
    assert!(LinearGradient::new(180.0, &two[..1]).is_none());
    assert!(LinearGradient::new(180.0, &[]).is_none());
    let nine = [stop(Color::WHITE, None); MAX_GRADIENT_STOPS + 1];
    assert!(LinearGradient::new(180.0, &nine).is_none());
    assert!(LinearGradient::new(180.0, &nine[..MAX_GRADIENT_STOPS]).is_some());
    for angle in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(LinearGradient::new(angle, &two).is_none());
    }
    for position in [f64::NAN, f64::INFINITY] {
        let stops = [stop(Color::WHITE, Some(position)), stop(Color::BLACK, None)];
        assert!(LinearGradient::new(180.0, &stops).is_none());
    }
}

#[test]
fn stop_fixup_follows_section_3_4_3() {
    let color = Color::WHITE;
    let cases: [(&[Option<f64>], &[f64]); 6] = [
        (&[None, None, None], &[0.0, 0.5, 1.0]),
        (&[None, Some(0.8), None, None], &[0.0, 0.8, 0.9, 1.0]),
        (&[Some(0.5), Some(0.2), None], &[0.5, 0.5, 1.0]),
        (
            &[Some(0.2), None, None, None, Some(0.8)],
            &[0.2, 0.35, 0.5, 0.65, 0.8],
        ),
        (&[None, Some(-0.5), None], &[0.0, 0.0, 1.0]),
        // A clamped stop sets the base the following run spreads from.
        (&[Some(0.6), None, Some(0.1), None], &[0.6, 0.6, 0.6, 1.0]),
    ];
    for (authored, expected) in cases {
        let stops: Vec<_> = authored
            .iter()
            .map(|position| stop(color, *position))
            .collect();
        let resolved = positions(&gradient(180.0, &stops));
        assert_eq!(resolved.len(), expected.len(), "{authored:?}");
        for (actual, wanted) in resolved.iter().zip(expected) {
            assert!(
                (actual - wanted).abs() <= FRACTION_TOLERANCE,
                "{authored:?}: {resolved:?} against {expected:?}"
            );
        }
    }
}

#[test]
fn interpolation_is_premultiplied_and_clamped_at_the_ends() {
    let clear_to_red = gradient(
        90.0,
        &[
            stop(Color::TRANSPARENT, None),
            stop(Color::rgb(255, 0, 0), None),
        ],
    );
    // Premultiplied: the transparent stop contributes no hue, so the
    // midpoint is half-transparent pure red, where straight interpolation
    // would darken it to (128, 0, 0, 128).
    assert_eq!(
        clear_to_red.color_at_fraction(0.5),
        Color::rgba(255, 0, 0, 128)
    );
    assert_eq!(clear_to_red.color_at_fraction(-3.0), Color::TRANSPARENT);
    assert_eq!(clear_to_red.color_at_fraction(0.0), Color::TRANSPARENT);
    assert_eq!(clear_to_red.color_at_fraction(4.0), Color::rgb(255, 0, 0));

    let ramp = gradient(
        90.0,
        &[
            stop(Color::rgb(0, 0, 0), Some(0.25)),
            stop(Color::rgb(200, 100, 40), Some(0.75)),
        ],
    );
    assert_eq!(ramp.color_at_fraction(0.1), Color::rgb(0, 0, 0));
    // A quarter of the way along the 0.25..0.75 span; every weight here is a
    // dyadic fraction, so the channels are exact.
    assert_eq!(ramp.color_at_fraction(0.375), Color::rgb(50, 25, 10));
    assert_eq!(ramp.color_at_fraction(0.9), Color::rgb(200, 100, 40));

    let hard = gradient(
        90.0,
        &[stop(Color::RED, Some(0.5)), stop(Color::BLUE, Some(0.5))],
    );
    // Coincident stops make an infinitesimal transition: the first color up
    // to the shared position, the last one past it.
    assert_eq!(hard.color_at_fraction(0.499_999), Color::RED);
    assert_eq!(hard.color_at_fraction(0.500_001), Color::BLUE);
}

#[test]
fn placement_matches_the_section_3_1_gradient_line() {
    let two = [stop(Color::WHITE, None), stop(Color::BLACK, None)];
    for size in [(40.0, 20.0), (7.0, 300.0), (1.0, 1.0)] {
        for step in 0..48_u32 {
            let degrees = f64::from(step) * 7.5 - 30.0;
            let gradient = gradient(degrees, &two);
            let placed = PlacedGradient::new(&gradient, (0.0, 0.0), size);
            for (column, row) in [(0_u32, 0_u32), (3, 11), (6, 0), (0, 19)] {
                let center = (f64::from(column) + 0.5, f64::from(row) + 0.5);
                let expected = reference_fraction(degrees, size, center);
                let actual = placed.fraction(column, row);
                assert!(
                    (actual - expected).abs() <= FRACTION_TOLERANCE,
                    "{degrees} deg over {size:?} at {center:?}: {actual} against {expected}"
                );
            }
            // The ends of the gradient line lie on the perpendiculars
            // through opposite corners: the corners span exactly [0, 1].
            let corners = [(0.0, 0.0), (size.0, 0.0), (0.0, size.1), size];
            let fractions = corners.map(|corner| reference_fraction(degrees, size, corner));
            let low = fractions.iter().copied().fold(f64::INFINITY, f64::min);
            let high = fractions.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            assert!(low.abs() <= FRACTION_TOLERANCE && (high - 1.0).abs() <= FRACTION_TOLERANCE);
        }
    }
}

#[test]
fn keyword_angles_point_as_specified() {
    let two = [stop(Color::WHITE, None), stop(Color::BLACK, None)];
    let size = (10.0, 10.0);
    // (angle, pixel at the white end, pixel at the black end)
    let cases = [
        (0.0, (5, 9), (5, 0)),
        (90.0, (0, 5), (9, 5)),
        (180.0, (5, 0), (5, 9)),
        (270.0, (9, 5), (0, 5)),
        (-90.0, (9, 5), (0, 5)),
        (450.0, (0, 5), (9, 5)),
    ];
    for (degrees, white, black) in cases {
        let gradient = gradient(degrees, &two);
        let placed = PlacedGradient::new(&gradient, (0.0, 0.0), size);
        assert!(placed.fraction(white.0, white.1) < 0.1, "{degrees}");
        assert!(placed.fraction(black.0, black.1) > 0.9, "{degrees}");
    }
}

#[test]
fn vertical_fill_matches_the_closed_form_per_row() {
    let mut fb = surface(12, 100, Color::RED);
    let white_to_black = gradient(180.0, &[stop(Color::WHITE, None), stop(Color::BLACK, None)]);
    fill_gradient(
        &mut fb,
        Rect::new(0, 0, 12, 100),
        CornerRadius::SQUARE,
        &white_to_black,
    );
    for row in 0..100 {
        // Opaque stops interpolate each channel as 255 (1 - t) at the pixel
        // center t = (row + 0.5) / 100; no row lands on a rounding tie.
        let level = (255.0 * (1.0 - (f64::from(row) + 0.5) / 100.0)).round();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a rounded channel in [0, 255] is a byte"
        )]
        let level = level as u8;
        for column in 0..12 {
            assert_eq!(
                fb.get_pixel(column, row),
                Color::rgb(level, level, level),
                "row {row}"
            );
        }
    }
}

#[test]
fn horizontal_fill_matches_the_closed_form_per_column() {
    let mut fb = surface(64, 3, Color::WHITE);
    let ramp = gradient(
        90.0,
        &[
            stop(Color::rgb(0, 0, 0), None),
            stop(Color::rgb(0, 0, 255), None),
        ],
    );
    fill_gradient(&mut fb, Rect::new(0, 0, 64, 3), CornerRadius::SQUARE, &ramp);
    for column in 0..64 {
        let level = (255.0 * (f64::from(column) + 0.5) / 64.0).round();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a rounded channel in [0, 255] is a byte"
        )]
        let level = level as u8;
        for row in 0..3 {
            assert_eq!(fb.get_pixel(column, row), Color::rgb(0, 0, level));
        }
    }
}

#[test]
fn a_single_color_gradient_paints_exactly_what_fill_rect_paints() {
    let colors = [Color::rgb(44, 82, 130), Color::rgba(26, 54, 93, 90)];
    let rects = [
        Rect::new(3, 2, 30, 20),
        Rect::new(-6, -4, 25, 40),
        Rect::new(1, 1, 1, 9),
    ];
    for color in colors {
        for rect in rects {
            for radius in [0, 5, 12] {
                for degrees in [0.0, 90.0, 135.0, 180.0, 212.5] {
                    let radius = CornerRadius::clamped(radius, rect);
                    let background = Color::rgb(240, 244, 248);
                    let mut expected = surface(36, 28, background);
                    let mut actual = surface(36, 28, background);
                    fill_rect(&mut expected, rect, radius, color);
                    let flat = gradient(
                        degrees,
                        &[stop(color, None), stop(color, Some(0.3)), stop(color, None)],
                    );
                    fill_gradient(&mut actual, rect, radius, &flat);
                    assert_eq!(
                        actual.pixels(),
                        expected.pixels(),
                        "{color:?} {rect:?} {radius:?} {degrees}"
                    );
                }
            }
        }
    }
}

#[test]
fn translucent_gradients_composite_each_pixel_over_the_surface() {
    let background = Color::rgb(250, 250, 250);
    let rect = Rect::new(2, 1, 20, 10);
    let fade = gradient(
        120.0,
        &[
            stop(Color::rgba(20, 40, 200, 0), None),
            stop(Color::rgba(20, 40, 200, 200), None),
        ],
    );
    let mut actual = surface(24, 12, background);
    fill_gradient(&mut actual, rect, CornerRadius::SQUARE, &fade);
    let mut expected = surface(24, 12, background);
    let placed = PlacedGradient::new(&fade, (2.0, 1.0), (20.0, 10.0));
    for row in 1..11_u32 {
        for column in 2..22_u32 {
            let x = i32::try_from(column).expect("small test column");
            let y = i32::try_from(row).expect("small test row");
            expected.blend_pixel(x, y, placed.color_at(column, row));
        }
    }
    assert_eq!(actual.pixels(), expected.pixels());
    // The fade really varies: the pixel beside its transparent start corner
    // keeps far more of the background than the one beside its end corner.
    let (start, end) = (actual.get_pixel(2, 1), actual.get_pixel(21, 10));
    assert!(start.r >= 240 && end.r <= 110, "{start:?} {end:?}");
}

#[test]
fn extreme_and_empty_geometry_clip_without_panicking() {
    let ramp = gradient(45.0, &[stop(Color::WHITE, None), stop(Color::BLACK, None)]);
    let background = Color::rgb(1, 2, 3);
    for rect in [
        Rect::new(0, 0, 0, 10),
        Rect::new(0, 0, 10, -1),
        Rect::new(100, 100, 5, 5),
        Rect::new(-40, -40, 5, 5),
    ] {
        let mut fb = surface(16, 16, background);
        fill_gradient(&mut fb, rect, CornerRadius::clamped(3, rect), &ramp);
        assert!(
            (0..16).all(|y| (0..16).all(|x| fb.get_pixel(x, y) == background)),
            "{rect:?}"
        );
    }
    // A box far larger than the surface paints only the visible window, and
    // that window sits in the middle of the ramp.
    let mut fb = surface(16, 16, background);
    let huge = Rect::new(-1_000_000, -1_000_000, 2_000_016, 2_000_016);
    fill_gradient(&mut fb, huge, CornerRadius::clamped(50, huge), &ramp);
    // Sixteen pixels span about 1e-5 of the line, so every visible pixel is
    // the opaque midpoint gray to within a level.
    for y in 0..16 {
        for x in 0..16 {
            let pixel = fb.get_pixel(x, y);
            assert!(
                (127..=128).contains(&pixel.r)
                    && pixel.r == pixel.g
                    && pixel.g == pixel.b
                    && pixel.a == 255,
                "({x}, {y}): {pixel:?}"
            );
        }
    }
}

#[test]
fn transparent_gradients_leave_the_surface_untouched() {
    let clear = gradient(
        30.0,
        &[
            stop(Color::TRANSPARENT, None),
            stop(Color::rgba(9, 9, 9, 0), None),
        ],
    );
    let background = Color::rgb(90, 80, 70);
    let mut fb = surface(8, 8, background);
    fill_gradient(
        &mut fb,
        Rect::new(0, 0, 8, 8),
        CornerRadius::clamped(3, Rect::new(0, 0, 8, 8)),
        &clear,
    );
    assert!((0..8).all(|y| (0..8).all(|x| fb.get_pixel(x, y) == background)));
}
