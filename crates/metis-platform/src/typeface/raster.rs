//! Exact-area coverage rasterization of flattened outlines.
//!
//! Each line segment is split wherever it crosses a pixel row or column, so
//! every piece lies inside one cell. A piece of signed height `dy` whose
//! average x within its cell is `m` covers `dy · (1 − m)` of that cell to its
//! right and `dy · m` of the next cell; accumulating those two terms and
//! prefix-summing each row gives, in every pixel, the signed area the closed
//! outline covers. The magnitude, clamped to one, is the coverage: contours of
//! one winding add, and a counter-wound hole subtracts.
//!
//! Accumulation adds the area of overlapping contours instead of taking their
//! union, so it is exact only for an outline whose contours do not overlap.
//! The simple glyphs of the embedded faces are overlap-free, as a test over
//! every glyph proves, but a composite may place components that overlap — a
//! ring touching an `A`, a cedilla crossing a `C`. An outline built from more
//! than one component is therefore rasterized by the nonzero winding rule on
//! [`NONZERO_SUBROWS`] rows per pixel, exact along x.

use super::glyf::Point;

/// Largest distance a flattened chord may stray from its quadratic, in
/// pixels.
///
/// An edge displaced by `t` changes the coverage of a pixel it crosses by at
/// most `t`, so `1/256` keeps flattening within one 8-bit level.
const FLATTENING_TOLERANCE: f64 = 1.0 / 256.0;

/// Flattened segments one outline may hold. A text glyph at the largest size
/// flattens to a few thousand; the bound stops a hostile outline, scaled up
/// through nested components, from flattening into billions.
const MAX_OUTLINE_LINES: usize = 1 << 18;

/// Sample rows per pixel for the nonzero winding rule.
///
/// Horizontal coverage is exact on every row, so the only error is the
/// midpoint rule's across a pixel's height: half a row, `1/128` of full
/// coverage, for each unit the row coverage varies down the pixel. A single
/// horizontal edge costs two 8-bit levels; a thin stroke with both edges in
/// one pixel costs twice that.
pub(super) const NONZERO_SUBROWS: u16 = 64;

/// Pixels one glyph's coverage may span. At the largest text size a glyph of
/// the embedded faces spans about one million; the bound admits two em
/// squares at that size and stops a hostile outline from sizing the buffers.
const MAX_RASTER_CELLS: u64 = 4 << 20;

/// Flattened outline segments and the decode scratch they are built from.
#[derive(Default)]
pub(super) struct Outline {
    lines: Vec<(Point, Point)>,
    /// Set when a segment was dropped at [`MAX_OUTLINE_LINES`].
    exceeded: bool,
    /// Simple glyphs contributing contours; more than one may overlap.
    components: u16,
    pub(super) flags: Vec<u8>,
    pub(super) coordinates: Vec<(i32, i32)>,
}

impl Outline {
    pub(super) fn clear(&mut self) {
        self.lines.clear();
        self.exceeded = false;
        self.components = 0;
    }

    /// The flattened segments.
    #[cfg(test)]
    pub(super) fn lines(&self) -> &[(Point, Point)] {
        &self.lines
    }

    /// Simple glyphs contributing contours so far.
    #[cfg(test)]
    pub(super) const fn components(&self) -> u16 {
        self.components
    }

    /// Records that one more simple glyph contributes contours.
    pub(super) fn begin_component(&mut self) {
        self.components = self.components.saturating_add(1);
    }

    /// Whether segments were dropped at the size bound.
    pub(super) const fn exceeded(&self) -> bool {
        self.exceeded
    }

    pub(super) fn line(&mut self, from: Point, to: Point) {
        if self.lines.len() >= MAX_OUTLINE_LINES {
            self.exceeded = true;
            return;
        }
        self.lines.push((from, to));
    }

    /// Flattens the quadratic from `from` through control `control` to `to`.
    ///
    /// A quadratic's second difference `d = from − 2·control + to` is
    /// constant, and a chord spanning parameter length `h` strays at most
    /// `|d|·h²/4` from the curve, so `n = ⌈√(|d| / (4·tolerance))⌉` equal
    /// steps keep every chord within the tolerance.
    pub(super) fn quadratic(&mut self, from: Point, control: Point, to: Point) {
        // Past the segment bound the outline is already rejected, so the
        // curve is not walked at all.
        if self.exceeded {
            return;
        }
        let (dx, dy) = (
            2.0f64.mul_add(-control.x, from.x) + to.x,
            2.0f64.mul_add(-control.y, from.y) + to.y,
        );
        let deviation = dx.hypot(dy);
        let steps = (deviation / (4.0 * FLATTENING_TOLERANCE)).sqrt().ceil();
        // Glyph curves span at most a few hundred pixels at the largest text
        // size, which bounds the step count far inside u16.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a finite nonnegative step count clamped to the u16 range"
        )]
        let steps = steps.clamp(1.0, f64::from(u16::MAX)) as u16;
        let mut previous = from;
        for step in 1..=steps {
            let t = f64::from(step) / f64::from(steps);
            let u = 1.0 - t;
            let point = Point {
                x: (u * u).mul_add(from.x, (2.0 * u * t).mul_add(control.x, t * t * to.x)),
                y: (u * u).mul_add(from.y, (2.0 * u * t).mul_add(control.y, t * t * to.y)),
            };
            self.line(previous, point);
            if self.exceeded {
                return;
            }
            previous = point;
        }
    }

    /// Total length of the flattened segments.
    #[cfg(test)]
    pub(super) fn perimeter(&self) -> f64 {
        self.lines
            .iter()
            .map(|(from, to)| (to.x - from.x).hypot(to.y - from.y))
            .sum()
    }

    /// Bounding box of the flattened outline in whole pixels, or `None` for an
    /// outline without area.
    pub(super) fn bounds(&self) -> Option<PixelBounds> {
        let mut bounds = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for (from, to) in &self.lines {
            for point in [from, to] {
                bounds = (
                    bounds.0.min(point.x),
                    bounds.1.min(point.y),
                    bounds.2.max(point.x),
                    bounds.3.max(point.y),
                );
            }
        }
        if !(bounds.0 < bounds.2 && bounds.1 < bounds.3) {
            return None;
        }
        let bounds = PixelBounds {
            left: pixel(bounds.0.floor())?,
            top: pixel(bounds.1.floor())?,
            right: pixel(bounds.2.ceil())?,
            bottom: pixel(bounds.3.ceil())?,
        };
        let span = |low: i32, high: i32| u64::try_from(i64::from(high) - i64::from(low)).ok();
        let cells =
            span(bounds.left, bounds.right)?.checked_mul(span(bounds.top, bounds.bottom)?)?;
        (cells <= MAX_RASTER_CELLS).then_some(bounds)
    }

    /// Rasterizes the outline over `bounds` into the canvas coverage, one
    /// value per pixel in row-major order.
    pub(super) fn rasterize(&self, bounds: PixelBounds, canvas: &mut Canvas) {
        if self.components > 1 {
            self.rasterize_nonzero(bounds, canvas);
        } else {
            self.accumulate_area(bounds, canvas);
        }
    }

    /// Exact signed-area accumulation, for outlines whose contours do not
    /// overlap.
    fn accumulate_area(&self, bounds: PixelBounds, canvas: &mut Canvas) {
        let width = bounds.width();
        let height = bounds.height();
        // One spare column receives the right-hand share of the last cell.
        let stride = width + 1;
        let Canvas {
            accumulation,
            coverage,
            crossings,
            windings: _,
        } = canvas;
        accumulation.clear();
        accumulation.resize(stride * height, 0.0);
        let origin = Point {
            x: f64::from(bounds.left),
            y: f64::from(bounds.top),
        };
        for (from, to) in &self.lines {
            accumulate(
                accumulation,
                crossings,
                stride,
                (width, height),
                offset(*from, origin),
                offset(*to, origin),
            );
        }
        coverage.clear();
        coverage.reserve(width * height);
        for row in accumulation.chunks_exact(stride) {
            let mut sum = 0.0;
            for cell in &row[..width] {
                sum += cell;
                coverage.push(sum.abs().min(1.0));
            }
        }
    }
}

impl Outline {
    /// Nonzero-winding coverage: on each sample row, the spans where the
    /// winding number is nonzero cover their exact horizontal overlap with
    /// each pixel.
    pub(super) fn rasterize_nonzero(&self, bounds: PixelBounds, canvas: &mut Canvas) {
        let width = bounds.width();
        let height = bounds.height();
        let Canvas {
            coverage,
            crossings,
            windings,
            ..
        } = canvas;
        coverage.clear();
        coverage.resize(width * height, 0.0);
        let share = 1.0 / f64::from(NONZERO_SUBROWS);
        let left = f64::from(bounds.left);
        for (row, pixels) in coverage.chunks_exact_mut(width).enumerate() {
            let top = f64::from(bounds.top)
                + f64::from(u32::try_from(row).expect("invariant: rows fit u32"));
            for sample in 0..NONZERO_SUBROWS {
                let y = (f64::from(sample) + 0.5).mul_add(share, top);
                windings.clear();
                for (from, to) in &self.lines {
                    let (low, high) = if from.y < to.y {
                        (from, to)
                    } else {
                        (to, from)
                    };
                    // Half-open in y, so a vertex shared by two segments is
                    // crossed once.
                    if y < low.y || y >= high.y {
                        continue;
                    }
                    let x = (y - low.y) / (high.y - low.y) * (high.x - low.x) + low.x;
                    windings.push((x - left, if from.y < to.y { 1 } else { -1 }));
                }
                windings.sort_by(|a, b| a.0.total_cmp(&b.0));
                crossings.clear();
                let mut winding = 0_i32;
                for (x, direction) in windings.iter() {
                    let before = winding;
                    winding += direction;
                    if (before == 0) != (winding == 0) {
                        crossings.push(*x);
                    }
                }
                for span in crossings.chunks_exact(2) {
                    add_span(pixels, span[0], span[1], share);
                }
            }
        }
        for value in coverage.iter_mut() {
            *value = value.min(1.0);
        }
    }
}

/// Adds `weight` times the overlap of `[start, end)` with each pixel of a row.
fn add_span(pixels: &mut [f64], start: f64, end: f64, weight: f64) {
    let width = pixels.len();
    let Some(first) = cell(start.max(0.0), width) else {
        return;
    };
    let last = cell((end - f64::EPSILON).max(0.0), width).unwrap_or(width - 1);
    for (index, pixel) in pixels.iter_mut().enumerate().take(last + 1).skip(first) {
        let column = f64::from(u32::try_from(index).expect("invariant: columns fit u32"));
        let overlap = end.min(column + 1.0) - start.max(column);
        if overlap > 0.0 {
            *pixel += weight * overlap;
        }
    }
}

/// Reusable rasterization buffers.
#[derive(Default)]
pub(super) struct Canvas {
    accumulation: Vec<f64>,
    /// Coverage of the last rasterized outline, row-major over its bounds.
    pub(super) coverage: Vec<f64>,
    crossings: Vec<f64>,
    windings: Vec<(f64, i32)>,
}

/// Whole-pixel bounds of a rasterized glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PixelBounds {
    pub(super) left: i32,
    pub(super) top: i32,
    pub(super) right: i32,
    pub(super) bottom: i32,
}

impl PixelBounds {
    pub(super) fn width(self) -> usize {
        usize::try_from(self.right - self.left).expect("invariant: bounds are ordered")
    }

    pub(super) fn height(self) -> usize {
        usize::try_from(self.bottom - self.top).expect("invariant: bounds are ordered")
    }
}

fn offset(point: Point, origin: Point) -> Point {
    Point {
        x: point.x - origin.x,
        y: point.y - origin.y,
    }
}

/// A whole pixel coordinate within the `i32` range.
fn pixel(value: f64) -> Option<i32> {
    if !value.is_finite() || value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
        return None;
    }
    // The guard bounds the value to the i32 range and it is already whole.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a whole value checked to lie in the i32 range"
    )]
    let pixel = value as i32;
    Some(pixel)
}

/// Adds one segment's signed area to the accumulation buffer.
fn accumulate(
    accumulation: &mut [f64],
    crossings: &mut Vec<f64>,
    stride: usize,
    (width, height): (usize, usize),
    from: Point,
    to: Point,
) {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    if dy == 0.0 {
        // A horizontal segment bounds no area.
        return;
    }
    // Parameters where the segment crosses a pixel column or row, so every
    // piece between consecutive crossings lies inside a single cell.
    crossings.clear();
    crossings.push(0.0);
    crossings.push(1.0);
    for (start, delta) in [(from.x, dx), (from.y, dy)] {
        if delta == 0.0 {
            continue;
        }
        let end = start + delta;
        let (low, high) = (start.min(end), start.max(end));
        let mut line = low.floor() + 1.0;
        while line < high {
            crossings.push((line - start) / delta);
            line += 1.0;
        }
    }
    crossings.sort_by(f64::total_cmp);
    for pair in crossings.windows(2) {
        let [begin, end] = [pair[0], pair[1]];
        if end <= begin {
            continue;
        }
        let piece_dy = dy * (end - begin);
        if piece_dy == 0.0 {
            continue;
        }
        let middle = f64::midpoint(begin, end);
        let (x, y) = (dx.mul_add(middle, from.x), dy.mul_add(middle, from.y));
        let Some(row) = cell(y, height) else {
            continue;
        };
        let (column, within) = if x < 0.0 {
            // Entirely left of the bitmap: the whole row to the right is
            // covered, which the first column carries.
            (0, 0.0)
        } else {
            match cell(x, width) {
                Some(column) => (column, x - x.floor()),
                // Right of the bitmap covers nothing visible.
                None => continue,
            }
        };
        let base = row * stride + column;
        accumulation[base] += piece_dy * (1.0 - within);
        accumulation[base + 1] += piece_dy * within;
    }
}

/// The cell index holding coordinate `value`, if it lies in `[0, extent)`.
fn cell(value: f64, extent: usize) -> Option<usize> {
    if value.is_nan() || value < 0.0 {
        return None;
    }
    // Pixel extents are small, so a nonnegative coordinate below the extent
    // floors to a valid index.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a nonnegative coordinate below a small extent floors to an index"
    )]
    let index = value.floor() as usize;
    (index < extent).then_some(index)
}

#[cfg(test)]
#[path = "raster_tests.rs"]
mod tests;
