//! The run kernel against the per-pixel path it replaces, bit for bit.

use super::super::GradientStop;
use super::*;
use crate::framebuffer::Color;

/// Non-axis angles in every quadrant, so the column step takes both signs.
fn angles() -> impl Iterator<Item = f64> {
    (0..33)
        .map(|index| 1.0 + 11.3 * f64::from(index))
        .chain([45.0, 135.0, 225.0, 315.0])
}

/// Opaque stop sets: the demo header's pair, coincident stops, stops outside
/// the line, and the most stops a gradient carries.
fn stop_sets() -> Vec<Vec<GradientStop>> {
    let stop = |color, position| GradientStop { color, position };
    let rgb = Color::rgb;
    vec![
        vec![stop(rgb(26, 54, 93), None), stop(rgb(44, 82, 130), None)],
        vec![
            stop(rgb(255, 0, 0), None),
            stop(rgb(0, 255, 0), Some(0.2)),
            stop(rgb(0, 0, 255), Some(0.2)),
            stop(rgb(250, 250, 5), None),
            stop(rgb(7, 7, 7), Some(0.9)),
        ],
        vec![
            stop(rgb(12, 200, 99), Some(-0.3)),
            stop(rgb(240, 16, 3), Some(0.5)),
            stop(rgb(1, 2, 250), Some(1.4)),
        ],
        (0..8u8)
            .map(|index| stop(rgb(index * 31, 255 - index * 29, index * 17), None))
            .collect(),
    ]
}

/// Box placements with fractional corners and extents.
const PLACEMENTS: [((f64, f64), (f64, f64)); 3] = [
    ((3.25, 1.5), (157.0, 23.0)),
    ((0.0, 0.0), (41.0, 97.0)),
    ((-12.5, 4.0), (300.0, 9.0)),
];

/// Columns per row: not a multiple of the run, so every row has a tail.
const COLUMNS: u32 = 157;

#[test]
fn runs_match_the_per_pixel_colors_bit_for_bit() {
    let mut interpolated_runs = 0_usize;
    for degrees in angles() {
        for stops in stop_sets() {
            let gradient = LinearGradient::new(degrees, &stops).expect("valid test gradient");
            assert!(gradient.is_opaque());
            for (corner, extent) in PLACEMENTS {
                let placed = PlacedGradient::new(&gradient, corner, extent);
                for row in 0..24 {
                    let left = row * 3;
                    let expected: Vec<u32> = (left..left + COLUMNS)
                        .map(|column| placed.packed_at(column, row))
                        .collect();
                    let mut actual = vec![0; expected.len()];
                    placed.fill_opaque_row(&mut actual, row, left);
                    assert_eq!(actual, expected, "{degrees} degrees, row {row}, {stops:?}");

                    // The interpolating path itself, on every run it takes.
                    let (runs, _) = actual.as_chunks_mut::<RUN>();
                    for (column, run) in (left..).step_by(RUN).zip(runs) {
                        let first = gradient.interval(placed.fraction(column, row));
                        let last = gradient.interval(placed.fraction(column + 7, row));
                        if let (Interval::Between(index), true) = (first, first == last) {
                            run.fill(0);
                            placed.interpolate(run, row, column, index);
                            let offset = usize::try_from(column - left).expect("small offset");
                            assert_eq!(run[..], expected[offset..offset + RUN]);
                            interpolated_runs += 1;
                        }
                    }
                }
            }
        }
    }
    // Most runs of these long rows lie within one interval.
    assert!(
        interpolated_runs > 20_000,
        "{interpolated_runs} interpolated runs"
    );
}
