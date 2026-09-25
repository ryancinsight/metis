//! Rows of an opaque gradient whose line is not vertical.
//!
//! Along a row the fraction is monotone in the column: it is two correctly
//! rounded fused multiply-adds, each monotone in its addend, and the column
//! enters only as an addend's term. The stop interval a fraction falls in is
//! monotone in the fraction because positions are nondecreasing. So when the
//! first and last pixel of a run of columns fall in the same interval, every
//! pixel between them does too, and the run interpolates without searching
//! the stops. Each pixel still takes exactly the color
//! [`LinearGradient::color_at_fraction`] gives it.

use super::super::paint::Paint;
use super::{LinearGradient, PlacedGradient, ResolvedStop, byte};
use crate::framebuffer::SourceOver;

/// Columns tested as one run: wide enough to amortize the two interval
/// lookups at its ends, narrow enough that a stop boundary costs few pixels
/// of the per-pixel fallback.
const RUN: usize = 8;

/// Column offsets within a run, exact in `f64`.
const OFFSETS: [f64; RUN] = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

/// Where a fraction falls relative to the stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Interval {
    /// At or before the first stop: its color.
    Before,
    /// Between the stop before `rest[index]` and `rest[index]`.
    Between(usize),
    /// At or after the last stop: its color.
    After,
}

impl LinearGradient {
    /// The interval [`Self::color_at_fraction`] interpolates `fraction` in.
    fn interval(&self, fraction: f64) -> Interval {
        if fraction <= self.first.position {
            return Interval::Before;
        }
        self.rest
            .iter()
            .position(|to| fraction < to.position)
            .map_or(Interval::After, Interval::Between)
    }

    /// The stops bounding [`Interval::Between`]`(index)`.
    fn bounds(&self, index: usize) -> (&ResolvedStop, &ResolvedStop) {
        let from = index
            .checked_sub(1)
            .map_or(&self.first, |previous| &self.rest[previous]);
        (from, &self.rest[index])
    }
}

impl PlacedGradient<'_> {
    /// Writes the opaque gradient over `pixels`, the columns of `row` from
    /// `left`.
    pub(super) fn fill_opaque_row(&self, pixels: &mut [u32], row: u32, left: u32) {
        let (runs, tail) = pixels.as_chunks_mut::<RUN>();
        let mut column = left;
        let run = u32::try_from(RUN).expect("invariant: a run is eight columns");
        for pixels in runs {
            let first = self.gradient.interval(self.fraction(column, row));
            let last = self
                .gradient
                .interval(self.fraction(column + (run - 1), row));
            match first {
                Interval::Between(index) if first == last => {
                    self.interpolate(pixels, row, column, index);
                }
                Interval::Before | Interval::After if first == last => {
                    pixels.fill(self.packed_at(column, row));
                }
                _ => self.fill_each(pixels, row, column),
            }
            column += run;
        }
        self.fill_each(tail, row, column);
    }

    /// Interpolates one run lying wholly in [`Interval::Between`]`(index)`.
    ///
    /// The same operations as [`LinearGradient::color_at_fraction`] on the
    /// color channels; alpha is 255 at both stops, so it interpolates to 255
    /// and the straight color is the premultiplied one.
    fn interpolate(&self, pixels: &mut [u32; RUN], row: u32, column: u32, index: usize) {
        let (from, to) = self.gradient.bounds(index);
        let [red, green, blue, _] = from.premultiplied;
        let [end_red, end_green, end_blue, _] = to.premultiplied;
        let (span_red, span_green, span_blue) = (end_red - red, end_green - green, end_blue - blue);
        let (row, column) = (f64::from(row), f64::from(column));
        for (pixel, offset) in pixels.iter_mut().zip(OFFSETS) {
            let fraction = self
                .step
                .1
                .mul_add(row, self.step.0.mul_add(column + offset, self.origin));
            let weight = (fraction - from.position) * from.inverse_span;
            *pixel = u32::from_be_bytes([
                255,
                byte(span_red.mul_add(weight, red)),
                byte(span_green.mul_add(weight, green)),
                byte(span_blue.mul_add(weight, blue)),
            ]);
        }
    }

    fn fill_each(&self, pixels: &mut [u32], row: u32, left: u32) {
        for (column, pixel) in (left..).zip(pixels) {
            *pixel = self.packed_at(column, row);
        }
    }

    fn packed_at(&self, column: u32, row: u32) -> u32 {
        SourceOver::new(self.color_at(column, row)).packed()
    }
}

#[cfg(test)]
#[path = "span_tests.rs"]
mod tests;
