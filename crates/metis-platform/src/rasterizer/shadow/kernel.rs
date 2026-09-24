//! Discrete Gaussian kernel with its running sum, and the sub-row profile
//! that resolves the rows crossing a corner arc.

use eunomia::FloatElement;

use crate::rasterizer::round_rect::SUBSAMPLES;

/// Standard deviations of kernel reach on each side of the centre tap.
///
/// The two dropped tails hold `2Φ(−3.5) ≈ 4.7e-4` of the Gaussian's mass,
/// below the `1/510` half step of an 8-bit channel, so truncation never moves
/// a rounded alpha by more than one level.
pub(super) const REACH_DEVIATIONS: f64 = 3.5;

/// Smallest product of the standard deviation and the sub-rows per arc row.
///
/// An arc row is integrated as `S` slabs, each with the arc's extent at its
/// centre. Near the flat apex of a large arc the edge is locally horizontal
/// and crosses a slab at an arbitrary height, which the slab replaces by a
/// step at its centre, so the error is first order in the slab height: at
/// most half the largest slab weight, `(2Φ(1/(2σS)) − 1) / 2`. At `σS = 8`
/// that is 2.49% of full scale, 6.35 levels, half the specification's 5%.
/// On small radii the arc is steep and the error is far smaller: 0.5 levels
/// on a 12-pixel radius at a one-pixel blur.
const SUBROW_RESOLUTION: f64 = 8.0;

/// Candidate sub-row counts; the largest serves σ = 0.5, the smallest blur,
/// at exactly the resolution.
const SUBROW_COUNTS: [u8; 5] = [1, 2, 4, 8, 16];

/// Sub-rows per arc row at zero blur, matching the coverage subsampling of
/// the rounded-rectangle fill so a hard shadow equals that fill exactly.
const COVERAGE_SUBROWS: u8 = 16;
const _: () = assert!(
    COVERAGE_SUBROWS as usize == SUBSAMPLES,
    "a hard shadow samples arc rows exactly as the fill does"
);

/// Discrete Gaussian taps with their running sum.
pub(super) struct Kernel {
    /// Taps on each side of the centre.
    pub(super) reach: i64,
    /// `running[i]` sums the first `i` taps, each the Gaussian's mass over
    /// one pixel, from offset `-reach` upward.
    running: Vec<f64>,
    /// Standard deviation, or `None` for the zero-blur box profile.
    deviation: Option<f64>,
    /// Slabs each arc row is integrated as.
    pub(super) subrows: usize,
    /// Vertical weight of slab `s` of the source row `d` rows above the
    /// output row, at `(d + reach) * subrows + s`.
    slabs: Vec<f64>,
}

/// Pixels the blur of radius `blur` reaches beyond the shape on each side.
pub(super) fn reach(blur: u32) -> i64 {
    // The blur is bounded by `BoxShadow::MAX_BLUR`, so the reach is a small
    // nonnegative integer before it is truncated.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the reach is at most 1.75 * MAX_BLUR, far inside i64"
    )]
    let reach = (REACH_DEVIATIONS * f64::from(blur) / 2.0).ceil() as i64;
    reach
}

impl Kernel {
    pub(super) fn new(blur: u32) -> Self {
        if blur == 0 {
            // The zero-width limit is the identity: the shadow is the mask.
            // Each slab carries an equal share of its own row, which is the
            // coverage average of the fill.
            let share = 1.0 / f64::from(COVERAGE_SUBROWS);
            return Self {
                reach: 0,
                running: vec![0.0, 1.0],
                deviation: None,
                subrows: usize::from(COVERAGE_SUBROWS),
                slabs: vec![share; usize::from(COVERAGE_SUBROWS)],
            };
        }
        let deviation = f64::from(blur) / 2.0;
        let reach = reach(blur);
        // One side is evaluated and mirrored, so opposite taps are equal to
        // the bit and mirrored corners receive identical sums.
        let side: Vec<f64> = (0..=reach)
            .map(|offset| {
                let centre = small_float(offset);
                normal_cdf((centre + 0.5) / deviation) - normal_cdf((centre - 0.5) / deviation)
            })
            .collect();
        let taps: Vec<f64> = side.iter().skip(1).rev().chain(&side).copied().collect();
        let mut running = Vec::with_capacity(taps.len() + 1);
        running.push(0.0);
        let mut sum = 0.0;
        for tap in &taps {
            sum += tap;
            running.push(sum);
        }
        // The smallest blur is one pixel, σ = 0.5, which the largest count
        // already resolves, so the search always finds a count.
        let count = SUBROW_COUNTS
            .into_iter()
            .find(|&count| deviation * f64::from(count) >= SUBROW_RESOLUTION)
            .unwrap_or(COVERAGE_SUBROWS);
        let height = f64::from(count);
        let slabs = (-reach..=reach)
            .flat_map(|offset| {
                (0..count).map(move |slab| {
                    // Output centre relative to the source row's top edge.
                    let centre = small_float(offset) + 0.5;
                    let top = f64::from(slab) / height;
                    normal_cdf((centre - top) / deviation)
                        - normal_cdf((centre - top - 1.0 / height) / deviation)
                })
            })
            .collect();
        let subrows = usize::from(count);
        Self {
            reach,
            running,
            deviation: Some(deviation),
            subrows,
            slabs,
        }
    }

    /// Vertical weight of `slab` in the source row `offset` rows above the
    /// output row, which lies within the reach.
    pub(super) fn slab(&self, offset: i64, slab: usize) -> f64 {
        self.slabs[index(offset + self.reach) * self.subrows + slab]
    }

    /// Horizontal profile of the extent `[low, high)` at pixel `column`.
    ///
    /// With a blur it is the Gaussian's mass over the extent, seen from the
    /// pixel centre, and exact in x; at zero blur it is the pixel's covered
    /// length, the fill's coverage.
    pub(super) fn profile(&self, low: f64, high: f64, column: f64) -> f64 {
        if high <= low {
            return 0.0;
        }
        match self.deviation {
            Some(deviation) => {
                let centre = column + 0.5;
                let limit = small_float(self.reach) + 0.5;
                let cdf = |distance: f64| {
                    // Beyond the reach the truncated kernel holds no mass,
                    // which also spares the error function there.
                    if distance >= limit {
                        1.0
                    } else if distance <= -limit {
                        0.0
                    } else {
                        normal_cdf(distance / deviation)
                    }
                };
                cdf(high - centre) - cdf(low - centre)
            }
            None => (high.min(column + 1.0) - low.max(column)).max(0.0),
        }
    }

    /// Mass of the whole truncated kernel.
    pub(super) fn total(&self) -> f64 {
        self.running[self.running.len() - 1]
    }

    /// Sum of the taps at offsets up to and including `offset`.
    pub(super) fn up_to(&self, offset: i64) -> f64 {
        if offset < -self.reach {
            0.0
        } else if offset >= self.reach {
            self.total()
        } else {
            self.running[index(offset + self.reach + 1)]
        }
    }

    /// Convolution of the interval indicator `[0, length)` at `position`.
    ///
    /// Tap `k` sees the interval when `0 <= position - k < length`, which is
    /// the tap range `(position - length, position]`.
    pub(super) fn interval(&self, position: i64, length: i64) -> f64 {
        self.up_to(position) - self.up_to(position - length)
    }
}

/// Standard normal cumulative distribution through the complementary error
/// function, which keeps the far tail free of cancellation.
pub(super) fn normal_cdf(z: f64) -> f64 {
    0.5 * FloatElement::erfc(-z * std::f64::consts::FRAC_1_SQRT_2)
}

/// Converts a kernel or shape coordinate to a float without rounding.
///
/// Coordinates are `i32` extents widened by at most the kernel reach, so their
/// magnitude stays far below `2^53` and every value converts exactly; a local
/// column can exceed `i32::MAX` by the reach, so the conversion is from `i64`.
pub(super) fn small_float(value: i64) -> f64 {
    debug_assert!(
        value.unsigned_abs() < 1 << 53,
        "coordinate {value} exceeds exact range"
    );
    #[expect(
        clippy::cast_precision_loss,
        reason = "coordinates are bounded by i32 extents plus the reach, far inside 2^53"
    )]
    let exact = value as f64;
    exact
}

/// Converts a nonnegative offset to a slice index.
pub(super) fn index(value: i64) -> usize {
    usize::try_from(value).expect("invariant: indices are offset to be nonnegative")
}
