//! Value-semantic tests for Gaussian box shadows.

use super::kernel::{Kernel, normal_cdf, small_float};
use super::*;
use crate::rasterizer::fill_rect;
use crate::rasterizer::round_rect::{
    RoundRect, RowSamples, SUBSAMPLES, composite_pixel, pixel_coverage, sample_row,
};

fn surface(width: u32, height: u32, background: Color) -> Framebuffer {
    let mut fb = Framebuffer::new(width, height).expect("test surface");
    fb.clear(background);
    fb
}

fn shadow(offset: (i32, i32), blur: u32, color: Color) -> BoxShadow {
    BoxShadow::new(offset.0, offset.1, blur, color).expect("blur within the bound")
}

/// Mass of the unit Gaussian of standard deviation `deviation` over `[low, high)`.
fn gaussian_mass(low: f64, high: f64, deviation: f64) -> f64 {
    normal_cdf(high / deviation) - normal_cdf(low / deviation)
}

/// Continuous blur of a rounded rectangle, sampled at a pixel centre.
///
/// The x integral is exact through the normal distribution; the y integral is
/// a midpoint rule of `steps` rows over the shape rows within eight standard
/// deviations, far past where the Gaussian contributes.
fn continuous_blur(outline: (f64, f64, f64, f64, f64), centre: (f64, f64), deviation: f64) -> f64 {
    let (left, top, right, bottom, radius) = outline;
    let low = top.max(centre.1 - 8.0 * deviation);
    let high = bottom.min(centre.1 + 8.0 * deviation);
    if high <= low {
        return 0.0;
    }
    let steps = 1000_u32;
    let step = (high - low) / f64::from(steps);
    (0..steps)
        .map(|index| {
            let row = (f64::from(index) + 0.5).mul_add(step, low);
            let depth = if row < top + radius {
                top + radius - row
            } else if row > bottom - radius {
                row - (bottom - radius)
            } else {
                0.0
            };
            let inset = radius - (radius * radius - depth * depth).max(0.0).sqrt();
            let horizontal =
                gaussian_mass(left + inset - centre.0, right - inset - centre.0, deviation);
            let vertical = (-0.5 * ((centre.1 - row) / deviation).powi(2)).exp()
                / (deviation * (2.0 * std::f64::consts::PI).sqrt());
            horizontal * vertical * step
        })
        .sum()
}

#[test]
fn normal_distribution_matches_published_values() {
    // Abramowitz and Stegun, Table 26.1: Φ(1) and the two-sided 5% point.
    assert!((normal_cdf(0.0) - 0.5).abs() < 1e-15);
    assert!((normal_cdf(1.0) - 0.841_344_746_068_542_9).abs() < 1e-12);
    assert!((normal_cdf(-1.959_963_984_540_054) - 0.025).abs() < 1e-12);
    assert!((normal_cdf(-3.5) - 2.326_290_790_355_25e-4).abs() < 1e-15);
}

#[test]
fn kernel_taps_hold_the_gaussian_mass_of_each_pixel() {
    for blur in [1, 2, 5, 16, BoxShadow::MAX_BLUR] {
        let kernel = Kernel::new(blur);
        let deviation = f64::from(blur) / 2.0;
        let reach = kernel.reach;
        let tap = |offset: i64| kernel.up_to(offset) - kernel.up_to(offset - 1);
        for offset in 0..=reach {
            let centre = small_float(offset);
            let mass = gaussian_mass(centre - 0.5, centre + 0.5, deviation);
            assert!(
                (tap(offset) - mass).abs() < 1e-12,
                "blur {blur} tap {offset}"
            );
            assert!((tap(offset) - tap(-offset)).abs() < 1e-15, "symmetric taps");
        }
        // The slabs of a row share out exactly that row's tap.
        for offset in -reach..=reach {
            let slabs: f64 = (0..kernel.subrows)
                .map(|slab| kernel.slab(offset, slab))
                .sum();
            assert!(
                (slabs - tap(offset)).abs() < 1e-12,
                "blur {blur} row {offset}"
            );
        }
        // Enough slabs that the product with the deviation reaches the
        // resolution the error bound is derived for.
        let slabs = f64::from(u8::try_from(kernel.subrows).expect("at most 16 slabs"));
        assert!(deviation * slabs >= 8.0, "blur {blur}: {slabs} slabs");
        // The truncated mass is the Gaussian's mass over the covered pixels.
        let covered = small_float(reach) + 0.5;
        let expected = gaussian_mass(-covered, covered, deviation);
        assert!((kernel.total() - expected).abs() < 1e-12, "blur {blur}");
        // The dropped tails stay below 2Φ(-3.5), under half an 8-bit step.
        assert!(
            1.0 - kernel.total() <= 2.0 * normal_cdf(-3.5),
            "blur {blur}"
        );
        for position in -reach - 3..reach + 12 {
            let direct: f64 = (-reach..=reach)
                .filter(|offset| (0..7).contains(&(position - offset)))
                .map(tap)
                .sum();
            assert!((kernel.interval(position, 7) - direct).abs() < 1e-12);
        }
    }
    let identity = Kernel::new(0);
    assert_eq!((identity.reach, identity.total()), (0, 1.0));
}

#[test]
fn shadow_description_rejects_an_unbounded_blur() {
    assert!(BoxShadow::new(0, 0, BoxShadow::MAX_BLUR, Color::BLACK).is_some());
    assert!(BoxShadow::new(0, 0, BoxShadow::MAX_BLUR + 1, Color::BLACK).is_none());
    let described = shadow((-3, 5), 7, Color::BLUE);
    assert_eq!(
        (
            described.offset_x(),
            described.offset_y(),
            described.blur(),
            described.color()
        ),
        (-3, 5, 7, Color::BLUE)
    );
}

#[test]
fn transparent_or_empty_shadows_paint_nothing() {
    let background = Color::rgb(210, 220, 230);
    let untouched = surface(40, 30, background);
    for (rect, color) in [
        (Rect::new(5, 5, 20, 10), Color::rgba(0, 0, 0, 0)),
        (Rect::new(5, 5, 0, 10), Color::BLACK),
        (Rect::new(5, 5, 20, 0), Color::BLACK),
        (Rect::new(500, 500, 20, 10), Color::BLACK),
    ] {
        let mut fb = surface(40, 30, background);
        draw_box_shadow(
            &mut fb,
            rect,
            CornerRadius::SQUARE,
            shadow((2, 2), 6, color),
        );
        assert_eq!(fb.pixels(), untouched.pixels(), "{rect:?}");
    }
}

#[test]
fn straight_edge_is_the_gaussian_sampled_at_pixel_centres() {
    let element = Rect::new(10, 20, 60, 60);
    let mut fb = surface(240, 100, Color::WHITE);
    draw_box_shadow(
        &mut fb,
        element,
        CornerRadius::SQUARE,
        shadow((120, 0), 8, Color::BLACK),
    );
    // The shadow spans [130, 190) x [20, 80); row 50 is 30 pixels from both
    // horizontal edges, beyond the 14-pixel reach, so only x varies.
    let deviation = 4.0;
    for x in 110..210 {
        let centre = f64::from(x) + 0.5;
        let expected = gaussian_mass(130.0 - centre, 190.0 - centre, deviation)
            * gaussian_mass(-29.5, 30.5, deviation);
        // Black over white leaves 255 minus the rounded alpha; truncation
        // moves the exact value by at most 255 * 2Φ(-3.5) ≈ 0.12 levels.
        let channel = f64::from(fb.get_pixel(x, 50).r);
        assert!(
            (channel - 255.0 * (1.0 - expected)).abs() <= 0.5 + 255.0 * 2.0 * normal_cdf(-3.5),
            "column {x}: {channel} against {}",
            255.0 * (1.0 - expected)
        );
    }
}

#[test]
fn corners_stay_within_the_specified_five_percent() {
    // CSS Backgrounds 3 §6.1.2 admits any image within 5% of the Gaussian
    // result per pixel; half a level of output rounding is added on top.
    let bound = 0.05 * 255.0 + 0.5;
    let element = Rect::new(4, 4, 60, 40);
    let radius = CornerRadius::clamped(12, element);
    for blur in [1, 2, 3, 4, 8, 16] {
        let mut fb = surface(240, 180, Color::WHITE);
        draw_box_shadow(
            &mut fb,
            element,
            radius,
            shadow((100, 80), blur, Color::BLACK),
        );
        let deviation = f64::from(blur) / 2.0;
        let reach = Kernel::new(blur).reach;
        let reach = i32::try_from(reach).expect("small reach");
        let outline = (104.0, 84.0, 164.0, 124.0, 12.0);
        let mut worst = 0.0_f64;
        // Every pixel of the small blurs, where the arc is sharpest.
        let stride = if blur <= 3 { 1 } else { 3 };
        for y in (84 - reach..84 + 12 + reach).step_by(stride) {
            for x in (104 - reach..104 + 12 + reach).step_by(stride) {
                let centre = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                let expected = 255.0 * (1.0 - continuous_blur(outline, centre, deviation));
                worst = worst.max((f64::from(fb.get_pixel(x, y).r) - expected).abs());
            }
        }
        assert!(
            worst <= bound,
            "blur {blur}: worst deviation {worst} levels"
        );
    }
}

#[test]
fn zero_blur_paints_the_offset_fill_outside_the_border_box() {
    let element = Rect::new(8, 6, 50, 36);
    let offset = (30, 20);
    for color in [Color::BLACK, Color::rgba(20, 30, 40, 150)] {
        let mut shadowed = surface(100, 80, Color::WHITE);
        draw_box_shadow(
            &mut shadowed,
            element,
            CornerRadius::clamped(10, element),
            shadow(offset, 0, color),
        );
        let mut filled = surface(100, 80, Color::WHITE);
        let moved = Rect::new(element.x + offset.0, element.y + offset.1, 50, 36);
        fill_rect(&mut filled, moved, CornerRadius::clamped(10, moved), color);
        let mut compared = 0;
        for y in 0..80 {
            for x in 0..100 {
                if element.contains(x, y) {
                    continue;
                }
                assert_eq!(
                    shadowed.get_pixel(x, y),
                    filled.get_pixel(x, y),
                    "({x}, {y})"
                );
                compared += 1;
            }
        }
        assert!(compared > 6000);
    }
}

#[test]
fn border_box_clips_the_shadow_it_casts() {
    let background = Color::rgb(90, 160, 220);
    let element = Rect::new(20, 20, 60, 40);
    let radius = CornerRadius::clamped(10, element);
    let mut fb = surface(100, 80, background);
    draw_box_shadow(&mut fb, element, radius, shadow((0, 0), 8, Color::BLACK));
    // Inside the arcs' inset the border box covers every pixel.
    for y in 20..60 {
        for x in 30..70 {
            assert_eq!(fb.get_pixel(x, y), background, "({x}, {y})");
        }
    }
    for y in 30..50 {
        for x in 20..80 {
            assert_eq!(fb.get_pixel(x, y), background, "({x}, {y})");
        }
    }
    // The straight edge darkens just outside and fades with distance.
    let near = fb.get_pixel(19, 40).r;
    let far = fb.get_pixel(12, 40).r;
    assert!(near < far && far < background.r, "{near} then {far}");
    // The corner pixel outside the arc receives shadow; its twin across the
    // horizontal mirror line receives the same amount.
    // Mirrored sums run in different orders, so a half-level rounding may flip.
    let corner = fb.get_pixel(20, 20).r;
    assert!(corner < background.r);
    assert!(corner.abs_diff(fb.get_pixel(20, 59).r) <= 1);
    assert!(corner.abs_diff(fb.get_pixel(79, 20).r) <= 1);
}

/// Direct evaluation of every source slab for every pixel, clipped by the
/// border box and composited pixel by pixel.
///
/// Arc and straight rows alike are integrated as the kernel's slabs, with the
/// Gaussian masses computed here from the normal distribution rather than
/// from the kernel's tables, and without truncation, so the reference shares
/// only the geometry and the slab count with the renderer.
fn naive_shadow(fb: &mut Framebuffer, element: Rect, radius: CornerRadius, described: BoxShadow) {
    let radius = CornerRadius::clamped(radius.pixels(), element);
    let kernel = Kernel::new(described.blur());
    let reach = kernel.reach;
    let outline =
        RoundRect::new(Rect::new(0, 0, element.width, element.height), radius).expect("shape");
    let border = RoundRect::new(element, radius).expect("border box");
    let (width, height) = (i64::from(element.width), i64::from(element.height));
    let deviation = f64::from(described.blur()) / 2.0;
    let slabs = kernel.subrows;
    let slab_height = 1.0 / small_float(i64::try_from(slabs).expect("at most 16 slabs"));
    let origin = (
        i64::from(element.x) + i64::from(described.offset_x()),
        i64::from(element.y) + i64::from(described.offset_y()),
    );
    let mut samples: RowSamples = [(0.0, 0.0); SUBSAMPLES];
    let mut unused: RowSamples = [(0.0, 0.0); SUBSAMPLES];
    for y in 0..fb.height() {
        let local_y = i64::from(y) - origin.1;
        if local_y < -reach - 1 || local_y > height + reach {
            continue;
        }
        sample_row(y, border, None, &mut samples, &mut unused);
        for x in 0..fb.width() {
            let local_x = i64::from(x) - origin.0;
            if local_x < -reach - 1 || local_x > width + reach {
                continue;
            }
            let column = small_float(local_x);
            let centre = (column + 0.5, small_float(local_y) + 0.5);
            let mut value = 0.0;
            for source in (local_y - reach).max(0)..(local_y + reach + 1).min(height) {
                for slab in 0..slabs {
                    let offset = small_float(i64::try_from(slab).expect("at most 16 slabs"));
                    let top = small_float(source) + offset * slab_height;
                    let (low, high) = outline.extent_at(top + slab_height / 2.0);
                    value += if described.blur() == 0 {
                        // The box profile: the covered length times the slab
                        // share of its row, which is the fill coverage.
                        (high.min(column + 1.0) - low.max(column)).max(0.0) * slab_height
                    } else if high > low {
                        gaussian_mass(top - centre.1, top + slab_height - centre.1, deviation)
                            * gaussian_mass(low - centre.0, high - centre.0, deviation)
                    } else {
                        0.0
                    };
                }
            }
            let covered = pixel_coverage(&samples, None, f64::from(x));
            composite_pixel(
                fb,
                y,
                f64::from(x),
                described.color(),
                value * (1.0 - covered),
            );
        }
    }
}

#[test]
fn separable_regions_match_the_naive_convolution() {
    let color = Color::rgba(10, 20, 30, 200);
    let cases = [
        // Narrow shape: no interior columns, arcs within reach of each other.
        (Rect::new(12, 10, 10, 24), 5, (3, 4), 9),
        // Height equal to twice the radius: every row crosses an arc.
        (Rect::new(10, 8, 40, 16), 8, (-4, 6), 5),
        // Wide shape with interior columns and straight rows.
        (Rect::new(6, 6, 50, 34), 7, (2, 3), 3),
        // Partly off the surface on two sides.
        (Rect::new(-15, -9, 40, 30), 6, (4, -2), 7),
        // Square corners and a one-pixel blur.
        (Rect::new(14, 12, 30, 20), 0, (5, 5), 1),
        // A one-pixel blur on an arc, integrated as eight slabs per row.
        (Rect::new(10, 9, 36, 26), 9, (3, 4), 1),
        // Offset by one pixel: only a one-pixel crescent escapes the clip.
        (Rect::new(8, 8, 40, 30), 4, (1, 1), 0),
    ];
    // Crossing the 1024-column tile boundary on a wide surface.
    let tiled = (Rect::new(990, 4, 70, 14), 6, (2, 1), 5);
    for ((element, radius, offset, blur), (width, height)) in cases
        .into_iter()
        .map(|case| (case, (64, 48)))
        .chain([(tiled, (1100, 24))])
    {
        let radius = CornerRadius::clamped(radius, element);
        let described = shadow(offset, blur, color);
        let mut painted = surface(width, height, Color::WHITE);
        draw_box_shadow(&mut painted, element, radius, described);
        let mut reference = surface(width, height, Color::WHITE);
        naive_shadow(&mut reference, element, radius, described);
        // The renderer truncates the kernel at 3.5 standard deviations on both
        // axes, which moves a value by at most 255 * 4 * Phi(-3.5), about 0.24
        // levels, so a rounding can flip by one level and no further.
        for (index, (left, right)) in painted.pixels().iter().zip(reference.pixels()).enumerate() {
            let (left, right) = (left.to_be_bytes(), right.to_be_bytes());
            for (a, b) in left.iter().zip(right) {
                assert!(a.abs_diff(b) <= 1, "{element:?} blur {blur} pixel {index}");
            }
        }
    }
}

#[test]
fn extreme_geometry_paints_only_what_is_visible() {
    let background = Color::rgb(200, 205, 210);
    let untouched = surface(24, 16, background);
    let extremes = [i32::MIN, -1_000_000, 0, 1_000_000, i32::MAX];
    for blur in [0, 1, BoxShadow::MAX_BLUR] {
        for radius in [0, 7, i32::MAX] {
            for &x in &extremes {
                for &offset in &extremes {
                    for width in [1, i32::MAX] {
                        let element = Rect::new(x, 2, width, 9);
                        let mut fb = surface(24, 16, background);
                        let described = shadow((offset, offset / 2), blur, Color::BLACK);
                        draw_box_shadow(
                            &mut fb,
                            element,
                            CornerRadius::clamped(radius, element),
                            described,
                        );
                        // A shadow whose blurred extent misses the surface
                        // leaves every pixel as it was.
                        let reach = i64::from(blur) * 2;
                        let left = i64::from(x) + i64::from(offset) - reach;
                        let right = i64::from(x) + i64::from(width) + i64::from(offset) + reach;
                        if right <= 0 || left >= 24 {
                            assert_eq!(
                                fb.pixels(),
                                untouched.pixels(),
                                "{element:?} {described:?}"
                            );
                        }
                    }
                }
            }
        }
    }
    // A border box covering the whole surface clips its own shadow away.
    let mut fb = surface(24, 16, background);
    draw_box_shadow(
        &mut fb,
        Rect::new(-1_000, -1_000, i32::MAX, i32::MAX),
        CornerRadius::SQUARE,
        shadow((3, 3), BoxShadow::MAX_BLUR, Color::BLACK),
    );
    assert_eq!(fb.pixels(), untouched.pixels());
}

#[test]
fn large_arcs_stay_within_the_first_order_slab_bound() {
    // Near the apex of a large arc the edge is nearly horizontal and crosses
    // each slab at an arbitrary height; the slab puts it at its centre, so
    // the error is at most half the largest slab weight, (2Φ(1/(2σS)) − 1)/2,
    // 2.49% of full scale at σS = 8. Half a level of output rounding is added.
    for radius in [200_i32, 1_000, 5_000] {
        for blur in [1, 2, 4] {
            let deviation = f64::from(blur) / 2.0;
            let slabs =
                f64::from(u8::try_from(Kernel::new(blur).subrows).expect("at most 16 slabs"));
            let bound =
                255.0 * (2.0 * normal_cdf(1.0 / (2.0 * deviation * slabs)) - 1.0) / 2.0 + 0.5;
            // The shadow sits 12 rows above its box, whose apex is at column
            // 40, so rows above the box show the shadow's own apex region.
            let element = Rect::new(40 - radius, 30, 2 * radius, 2 * radius);
            let mut fb = surface(80, 30, Color::WHITE);
            draw_box_shadow(
                &mut fb,
                element,
                CornerRadius::clamped(radius, element),
                shadow((0, -12), blur, Color::BLACK),
            );
            let (left, top) = (f64::from(40 - radius), 18.0);
            let size = f64::from(2 * radius);
            let outline = (left, top, left + size, top + size, f64::from(radius));
            let mut worst = 0.0_f64;
            for y in 12..28 {
                for x in 0..80 {
                    let centre = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                    let expected = 255.0 * (1.0 - continuous_blur(outline, centre, deviation));
                    worst = worst.max((f64::from(fb.get_pixel(x, y).r) - expected).abs());
                }
            }
            assert!(
                worst <= bound,
                "radius {radius} blur {blur}: {worst} levels against {bound}"
            );
        }
    }
}
