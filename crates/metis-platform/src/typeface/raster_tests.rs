//! Exact-area laws for the coverage rasterizer.

use super::*;

fn point(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn polygon(outline: &mut Outline, points: &[Point]) {
    for (index, from) in points.iter().enumerate() {
        outline.line(*from, points[(index + 1) % points.len()]);
    }
}

fn coverage(outline: &Outline) -> (PixelBounds, Vec<f64>) {
    let bounds = outline.bounds().expect("an outline with area");
    let mut canvas = Canvas::default();
    outline.rasterize(bounds, &mut canvas);
    (bounds, canvas.coverage)
}

/// Shoelace area of a closed polygon.
fn shoelace(points: &[Point]) -> f64 {
    let twice: f64 = (0..points.len())
        .map(|index| {
            let (a, b) = (points[index], points[(index + 1) % points.len()]);
            a.x * b.y - b.x * a.y
        })
        .sum();
    twice.abs() / 2.0
}

/// Deterministic coordinates in `[0, span)`.
struct Sequence(u64);

impl Sequence {
    fn next(&mut self, span: f64) -> f64 {
        // Knuth's MMIX linear congruential constants.
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let unit =
            f64::from(u32::try_from(self.0 >> 40).expect("24 bits")) / f64::from(1_u32 << 24);
        unit * span
    }
}

#[test]
fn an_axis_aligned_square_covers_exactly_its_overlap_with_each_pixel() {
    let mut outline = Outline::default();
    let square = [
        point(1.25, 1.5),
        point(3.75, 1.5),
        point(3.75, 4.0),
        point(1.25, 4.0),
    ];
    polygon(&mut outline, &square);
    let (bounds, values) = coverage(&outline);
    assert_eq!(
        bounds,
        PixelBounds {
            left: 1,
            top: 1,
            right: 4,
            bottom: 4
        }
    );
    let width = bounds.width();
    // Columns 1..4 cover 0.75, 1, 0.75 horizontally; rows cover 0.5, 1, 1.
    let horizontal = [0.75, 1.0, 0.75];
    let vertical = [0.5, 1.0, 1.0];
    for (row, fraction_y) in vertical.iter().enumerate() {
        for (column, fraction_x) in horizontal.iter().enumerate() {
            let value = values[row * width + column];
            assert!(
                (value - fraction_x * fraction_y).abs() < 1e-12,
                "({column}, {row}) {value}"
            );
        }
    }
}

#[test]
fn coverage_sums_to_the_polygon_area_for_either_winding() {
    let mut sequence = Sequence(7);
    for _ in 0..200 {
        let points: Vec<Point> = (0..3)
            .map(|_| point(sequence.next(19.0) + 0.5, sequence.next(13.0) + 0.5))
            .collect();
        let area = shoelace(&points);
        if area < 1e-6 {
            continue;
        }
        for winding in [points.clone(), points.iter().rev().copied().collect()] {
            let mut outline = Outline::default();
            polygon(&mut outline, &winding);
            let (_, values) = coverage(&outline);
            let total: f64 = values.iter().sum();
            // Each pixel's coverage is an exact area, so only rounding in the
            // accumulation separates the total from the shoelace area.
            assert!((total - area).abs() < 1e-9, "{total} against {area}");
            assert!(values.iter().all(|value| (0.0..=1.0).contains(value)));
        }
    }
}

#[test]
fn a_counter_wound_hole_leaves_its_interior_uncovered() {
    let mut outline = Outline::default();
    polygon(
        &mut outline,
        &[
            point(0.0, 0.0),
            point(8.0, 0.0),
            point(8.0, 8.0),
            point(0.0, 8.0),
        ],
    );
    polygon(
        &mut outline,
        &[
            point(2.0, 2.0),
            point(2.0, 6.0),
            point(6.0, 6.0),
            point(6.0, 2.0),
        ],
    );
    let (bounds, values) = coverage(&outline);
    let width = bounds.width();
    for row in 0..8 {
        for column in 0..8 {
            let inside_hole = (2..6).contains(&row) && (2..6).contains(&column);
            let expected = if inside_hole { 0.0 } else { 1.0 };
            let value = values[row * width + column];
            assert!(
                (value - expected).abs() < 1e-12,
                "({column}, {row}) {value}"
            );
        }
    }
    let total: f64 = values.iter().sum();
    assert!((total - 48.0).abs() < 1e-12);
}

#[test]
fn flattened_quadratics_stay_within_the_tolerance_of_the_curve() {
    let mut sequence = Sequence(11);
    for _ in 0..100 {
        let [from, control, to] =
            [0, 1, 2].map(|_| point(sequence.next(200.0), sequence.next(200.0)));
        let mut outline = Outline::default();
        outline.quadratic(from, control, to);
        let curve = |t: f64| {
            let u = 1.0 - t;
            point(
                u * u * from.x + 2.0 * u * t * control.x + t * t * to.x,
                u * u * from.y + 2.0 * u * t * control.y + t * t * to.y,
            )
        };
        // Sample the curve densely; every sample must lie within the
        // tolerance of some chord.
        for sample in 0..=400 {
            let at = curve(f64::from(sample) / 400.0);
            let nearest = outline
                .lines
                .iter()
                .map(|(a, b)| distance_to_segment(at, *a, *b))
                .fold(f64::INFINITY, f64::min);
            assert!(nearest <= FLATTENING_TOLERANCE + 1e-9, "{nearest}");
        }
    }
}

fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length = dx * dx + dy * dy;
    let t = if length == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / length).clamp(0.0, 1.0)
    };
    (p.x - (a.x + t * dx)).hypot(p.y - (a.y + t * dy))
}

#[test]
fn outlines_beyond_the_segment_bound_are_reported() {
    let mut outline = Outline::default();
    for index in 0..=MAX_OUTLINE_LINES {
        let y = f64::from(u32::try_from(index).expect("small"));
        outline.line(point(0.0, y), point(1.0, y + 1.0));
    }
    assert!(outline.exceeded());
    assert_eq!(outline.lines.len(), MAX_OUTLINE_LINES);
    // Once rejected, a curve is not walked: it adds nothing.
    outline.quadratic(
        point(0.0, 0.0),
        point(5_000.0, 90_000.0),
        point(9_000.0, 0.0),
    );
    assert_eq!(outline.lines.len(), MAX_OUTLINE_LINES);
    outline.clear();
    assert!(!outline.exceeded() && outline.lines.is_empty());
}

#[test]
fn outlines_beyond_the_cell_bound_have_no_raster_bounds() {
    let mut outline = Outline::default();
    polygon(
        &mut outline,
        &[
            point(0.0, 0.0),
            point(1_000.0, 0.0),
            point(1_000.0, 1_000.0),
        ],
    );
    assert!(outline.bounds().is_some(), "a million cells fit");
    outline.clear();
    // 2049 x 2049 pixels exceed the four-million-cell bound.
    polygon(
        &mut outline,
        &[
            point(0.0, 0.0),
            point(2_049.0, 0.0),
            point(2_049.0, 2_049.0),
        ],
    );
    assert!(outline.bounds().is_none());
}
