//! Antialiased rounded-rectangle scanline coverage.
//!
//! One scanline routine serves both the filled shape and its border: a border
//! is the outer shape minus an inset inner shape, so the two cases differ only
//! by whether an inner shape is supplied. Horizontal coverage is exact within
//! each sampled row and the vertical direction is integrated over a fixed
//! number of subsamples. Straight edges land on integer coordinates, so only
//! the corner arcs produce fractional coverage and a square radius reduces to
//! the unrounded span fill.

use super::paint::Paint;
use crate::framebuffer::{Color, Framebuffer, Rect, SourceOver};

/// Vertical subsamples integrated per device row.
pub(super) const SUBSAMPLES: usize = 16;
/// Reciprocal of [`SUBSAMPLES`], written as a literal so coverage needs no cast.
const SUBSAMPLE_RECIPROCAL: f64 = 1.0 / 16.0;
const _: () = assert!(
    SUBSAMPLES == 16,
    "the reciprocal literal tracks the subsample count"
);

/// Corner radius in device pixels, bounded by the rectangle it rounds.
///
/// A radius never exceeds half the shorter side, so opposite corners cannot
/// overlap and the straight edge between them keeps a nonnegative extent.
///
/// # Examples
///
/// ```
/// use metis_platform::framebuffer::Rect;
/// use metis_platform::rasterizer::CornerRadius;
///
/// let rect = Rect::new(0, 0, 40, 24);
/// assert_eq!(CornerRadius::clamped(8, rect).pixels(), 8);
/// // Half of the shorter side is the ceiling.
/// assert_eq!(CornerRadius::clamped(100, rect).pixels(), 12);
/// assert!(CornerRadius::SQUARE.is_square());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CornerRadius(i32);

impl CornerRadius {
    /// A square corner.
    pub const SQUARE: Self = Self(0);

    /// Clamps a requested radius to the rectangle it rounds.
    ///
    /// A nonpositive request or an empty rectangle produces a square corner.
    #[must_use]
    pub fn clamped(requested: i32, rect: Rect) -> Self {
        if requested <= 0 || rect.width <= 0 || rect.height <= 0 {
            return Self::SQUARE;
        }
        Self(requested.min(rect.width.min(rect.height) / 2))
    }

    /// The effective radius in device pixels.
    #[must_use]
    pub const fn pixels(self) -> i32 {
        self.0
    }

    /// Reports a corner that needs no arc coverage.
    #[must_use]
    pub const fn is_square(self) -> bool {
        self.0 == 0
    }
}

/// A rounded rectangle in continuous device coordinates.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RoundRect {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    radius: f64,
}

impl RoundRect {
    /// Builds the shape, or `None` when the rectangle encloses no area.
    pub(crate) fn new(rect: Rect, radius: CornerRadius) -> Option<Self> {
        if rect.width <= 0 || rect.height <= 0 {
            return None;
        }
        let left = f64::from(rect.x);
        let top = f64::from(rect.y);
        Some(Self {
            left,
            top,
            right: left + f64::from(rect.width),
            bottom: top + f64::from(rect.height),
            radius: f64::from(CornerRadius::clamped(radius.pixels(), rect).pixels()),
        })
    }

    /// Insets every side by `amount`, shrinking the radius with it.
    ///
    /// Returns `None` when the inset consumes the shape.
    pub(crate) fn inset(self, amount: f64) -> Option<Self> {
        let inset = Self {
            left: self.left + amount,
            top: self.top + amount,
            right: self.right - amount,
            bottom: self.bottom - amount,
            radius: (self.radius - amount).max(0.0),
        };
        if inset.right <= inset.left || inset.bottom <= inset.top {
            return None;
        }
        Some(inset)
    }

    /// The half-open horizontal extent at continuous row `y`.
    ///
    /// Rows outside the shape report an empty extent.
    pub(super) fn extent_at(self, y: f64) -> (f64, f64) {
        if y < self.top || y >= self.bottom {
            return (0.0, 0.0);
        }
        let depth = if y < self.top + self.radius {
            self.top + self.radius - y
        } else if y > self.bottom - self.radius {
            y - (self.bottom - self.radius)
        } else {
            0.0
        };
        if depth <= 0.0 {
            return (self.left, self.right);
        }
        // Horizontal inset of the corner arc at this depth from its centre row.
        let inset = self.radius - (self.radius * self.radius - depth * depth).max(0.0).sqrt();
        (self.left + inset, self.right - inset)
    }
}

/// Half-open horizontal extents sampled once per subsample row.
pub(super) type RowSamples = [(f64, f64); SUBSAMPLES];

/// Bounds collected while sampling one device row.
#[derive(Debug, Clone, Copy)]
pub(super) struct RowBounds {
    /// Leftmost and rightmost column any outer subsample reaches.
    pub(super) touched: (f64, f64),
    /// Columns every outer subsample covers completely.
    pub(super) solid: (f64, f64),
    /// Columns any inner extent reaches, empty when there is no inner shape.
    hole: Option<(f64, f64)>,
    /// Columns every inner subsample covers completely.
    ///
    /// An inner shape is always an inset of the outer one, so a pixel the
    /// inner shape covers completely is covered completely by the outer shape
    /// too and its coverage is exactly zero. Skipping that run keeps the
    /// hollow interior of a border out of the per-pixel path, which otherwise
    /// costs one coverage integration per interior column on every row.
    hollow: Option<(f64, f64)>,
}

/// Length of the intersection between a half-open extent and pixel `column`.
fn overlap(extent: (f64, f64), column: f64) -> f64 {
    (extent.1.min(column + 1.0) - extent.0.max(column)).max(0.0)
}

/// Samples both shapes across one device row.
pub(super) fn sample_row(
    row: u32,
    outer: RoundRect,
    inner: Option<RoundRect>,
    outer_samples: &mut RowSamples,
    inner_samples: &mut RowSamples,
) -> RowBounds {
    let mut touched = (f64::INFINITY, f64::NEG_INFINITY);
    let mut solid = (f64::NEG_INFINITY, f64::INFINITY);
    let mut hole = (f64::INFINITY, f64::NEG_INFINITY);
    let mut hollow = (f64::NEG_INFINITY, f64::INFINITY);
    let top = f64::from(row);
    for index in 0..SUBSAMPLES {
        let step = u32::try_from(index).expect("invariant: the subsample count fits u32");
        let sample_y = (f64::from(step) + 0.5).mul_add(SUBSAMPLE_RECIPROCAL, top);
        let extent = outer.extent_at(sample_y);
        outer_samples[index] = extent;
        // An empty extent leaves the solid interval empty for the whole row.
        solid = (solid.0.max(extent.0), solid.1.min(extent.1));
        if extent.1 > extent.0 {
            touched = (touched.0.min(extent.0), touched.1.max(extent.1));
        }
        let Some(inner) = inner else {
            continue;
        };
        let extent = inner.extent_at(sample_y);
        inner_samples[index] = extent;
        // An empty inner extent on any subsample collapses the hollow run, so
        // a row the inner shape only partly spans keeps its per-pixel path.
        hollow = (hollow.0.max(extent.0), hollow.1.min(extent.1));
        if extent.1 > extent.0 {
            hole = (hole.0.min(extent.0), hole.1.max(extent.1));
        }
    }
    RowBounds {
        touched,
        solid,
        hole: (hole.1 > hole.0).then_some(hole),
        hollow: (inner.is_some() && hollow.1 > hollow.0).then_some(hollow),
    }
}

/// Mean covered fraction of one pixel across the sampled rows.
pub(super) fn pixel_coverage(outer: &RowSamples, inner: Option<&RowSamples>, column: f64) -> f64 {
    let mut total = 0.0;
    for index in 0..SUBSAMPLES {
        let mut covered = overlap(outer[index], column);
        if let Some(inner) = inner {
            covered -= overlap(inner[index], column);
        }
        total += covered.max(0.0);
    }
    total * SUBSAMPLE_RECIPROCAL
}

/// Converts a clamped nonnegative coordinate to a surface index.
///
/// Both the column walk and the row bounds use this, so the name states the
/// axis-neutral job rather than either caller.
fn surface_index(value: f64) -> Option<u32> {
    if !value.is_finite() || value < 0.0 || value > f64::from(u32::MAX) {
        return None;
    }
    // The guard above bounds the value to the unsigned range, and flooring a
    // nonnegative coordinate is exactly the wanted column.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the guard bounds the value to the u32 range before flooring"
    )]
    let column = value.trunc() as u32;
    Some(column)
}

/// Composites the area inside `outer` and outside `inner` with `paint`.
///
/// Supplying `None` for `inner` fills the whole shape. Coverage scales the
/// paint's alpha at each partially covered pixel, and fully covered runs go
/// to [`Paint::fill_run`], so a fully covered pixel composites exactly as an
/// unrounded fill of the same paint does.
pub(crate) fn composite_shape(
    fb: &mut Framebuffer,
    outer: RoundRect,
    inner: Option<RoundRect>,
    paint: &impl Paint,
) {
    if paint.is_transparent() {
        return;
    }
    let clip = fb.clip();
    let (low, high) = (f64::from(clip.left()), f64::from(clip.right()));
    let first_row = outer.top.max(f64::from(clip.top()));
    let last_row = outer.bottom.min(f64::from(clip.bottom())).max(first_row);
    let (Some(first_row), Some(last_row)) =
        (surface_index(first_row), surface_index(last_row.ceil()))
    else {
        return;
    };
    let mut outer_samples: RowSamples = [(0.0, 0.0); SUBSAMPLES];
    let mut inner_samples: RowSamples = [(0.0, 0.0); SUBSAMPLES];
    for row in first_row..last_row.min(clip.bottom()) {
        let bounds = sample_row(row, outer, inner, &mut outer_samples, &mut inner_samples);
        if bounds.touched.1 <= bounds.touched.0 {
            continue;
        }
        let clamp = |value: f64| value.clamp(low, high);
        let start = clamp(bounds.touched.0.floor());
        let end = clamp(bounds.touched.1.ceil());
        let solid_start = clamp(bounds.solid.0.ceil());
        let solid_end = clamp(bounds.solid.1.floor()).max(solid_start);
        // Fully covered pixels split around the columns the hole can reach.
        let (hole_start, hole_end) = bounds.hole.map_or((solid_end, solid_end), |(low, high)| {
            (clamp(low.floor()), clamp(high.ceil()))
        });
        let runs = [
            (solid_start, solid_end.min(hole_start)),
            (solid_start.max(hole_end), solid_end),
        ];
        // Columns the inner shape covers completely contribute no coverage.
        let hollow = bounds.hollow.map(|(low, high)| {
            let low = clamp(low.ceil());
            (low, clamp(high.floor()).max(low))
        });
        let inner_samples = inner.map(|_| &inner_samples);
        let mut column = start;
        while column < end {
            if let Some((_, run_end)) = runs
                .iter()
                .find(|(low, high)| high > low && column >= *low && column < *high)
            {
                let stop = run_end.min(end);
                let bounded = "invariant: a run is clamped to the surface before it is filled";
                paint.fill_run(
                    fb,
                    row,
                    surface_index(column).expect(bounded),
                    surface_index(stop).expect(bounded),
                );
                column = stop;
                continue;
            }
            if let Some((low, high)) = hollow
                && high > low
                && column >= low
                && column < high
            {
                column = high.min(end);
                continue;
            }
            let coverage = pixel_coverage(&outer_samples, inner_samples, column);
            if coverage > 0.0
                && let Some(x) = surface_index(column)
            {
                composite_pixel(fb, row, column, paint.color_at(x, row), coverage);
            }
            column += 1.0;
        }
    }
}

/// Composites one partially covered pixel.
pub(super) fn composite_pixel(
    fb: &mut Framebuffer,
    row: u32,
    column: f64,
    color: Color,
    coverage: f64,
) {
    if coverage <= 0.0 {
        return;
    }
    let Some(x) = surface_index(column) else {
        return;
    };
    let source = SourceOver::covering(color, coverage);
    if source.is_transparent() {
        return;
    }
    fb.composite_span(row, x, x + 1, source);
}

#[cfg(test)]
#[path = "round_rect_tests.rs"]
mod tests;
