//! The blurred shape, evaluated row by row over one tile of columns.
//!
//! Output row `y` at column `x` sums, over the source rows in the vertical
//! window, the source row's horizontal profile weighted by the Gaussian's
//! mass over that row. A straight source row spans the full width, so its
//! profile is the kernel's interval sum `E(x)` and the straight rows together
//! contribute `E(x)` times one interval of the running sum. An arc row's
//! extent varies across its height, so it is integrated as `S` slabs, each
//! with the arc's extent at its centre, an exact profile along x and the
//! Gaussian's exact mass over the slab along y.
//!
//! Only arc rows are stored, in a ring keyed by their ordinal among the arc
//! rows. Top-band ordinals run straight into bottom-band ones, so the arc rows
//! any window reaches form one contiguous run of at most `min(2K + 1, 2r)`
//! ordinals and never collide in a ring of that many slots.

use std::ops::Range;

use super::kernel::{Kernel, index, small_float};
use crate::rasterizer::round_rect::RoundRect;

/// Shape geometry in coordinates local to the shadow's top-left corner.
pub(super) struct Shape {
    pub(super) width: i64,
    pub(super) height: i64,
    pub(super) radius: i64,
    pub(super) outline: RoundRect,
}

impl Shape {
    /// Position of an arc row among the arc rows, top band first.
    fn arc_ordinal(&self, row: i64) -> i64 {
        if row < self.radius {
            row
        } else {
            self.radius + row - (self.height - self.radius)
        }
    }
}

/// The blurred shape over one tile of visible columns.
pub(super) struct Field<'shape> {
    kernel: &'shape Kernel,
    shape: &'shape Shape,
    /// Local columns `[low, high)` whose horizontal pass is the full mass.
    interior: Range<i64>,
    /// Tile columns left of the interior.
    left: Range<i64>,
    /// Tile columns right of the interior.
    right: Range<i64>,
    /// Local column of each stored position: `left` then `right`.
    columns: Vec<i64>,
    /// Arc-row profiles: slot, then slab, then column position.
    profiles: Vec<f64>,
    /// Local arc row held by each slot.
    tags: Vec<Option<i64>>,
    /// Values of the current row at each stored position.
    values: Vec<f64>,
}

impl<'shape> Field<'shape> {
    /// Prepares evaluation; [`Self::set_tile`] selects the columns.
    pub(super) fn new(kernel: &'shape Kernel, shape: &'shape Shape) -> Self {
        let reach = kernel.reach;
        // Columns whose kernel window sees only full rows of the shape.
        let low = shape.radius + reach;
        let high = shape.width - shape.radius - reach;
        let interior = if low < high { low..high } else { 0..0 };
        let slots = index((2 * reach + 1).min(2 * shape.radius));
        Self {
            kernel,
            shape,
            interior,
            left: 0..0,
            right: 0..0,
            columns: Vec::new(),
            profiles: Vec::new(),
            tags: vec![None; slots],
            values: Vec::new(),
        }
    }

    /// Local columns whose value is [`Self::interior_value`].
    pub(super) fn interior(&self) -> Range<i64> {
        self.interior.clone()
    }

    /// Restricts evaluation to the local columns `tile`, reusing storage.
    pub(super) fn set_tile(&mut self, tile: Range<i64>) {
        (self.left, self.right) = if self.interior.is_empty() {
            (tile.clone(), tile.end..tile.end)
        } else {
            let split = tile.end.min(self.interior.start).max(tile.start);
            let resume = tile.start.max(self.interior.end).min(tile.end);
            (tile.start..split, resume..tile.end)
        };
        self.columns.clear();
        self.columns
            .extend(self.left.clone().chain(self.right.clone()));
        let stored = self.columns.len();
        self.values.clear();
        self.values.resize(stored, 0.0);
        self.profiles.clear();
        self.profiles
            .resize(self.tags.len() * self.kernel.subrows * stored, 0.0);
        self.tags.fill(None);
    }

    /// Value at an interior column: the full horizontal mass times the
    /// vertical convolution of the shape's row interval.
    pub(super) fn interior_value(&self, row: i64) -> f64 {
        self.kernel.total() * self.kernel.interval(row, self.shape.height)
    }

    /// Evaluates output row `row` at every stored column.
    pub(super) fn evaluate(&mut self, row: i64) {
        let (kernel, shape) = (self.kernel, self.shape);
        let radius = shape.radius;
        // Summed taps over the straight source rows `[r, h - r)`.
        let straight = kernel.interval(row - radius, shape.height - 2 * radius);
        for (value, &column) in self.values.iter_mut().zip(&self.columns) {
            *value = straight * kernel.interval(column, shape.width);
        }
        let reach = kernel.reach;
        let stored = self.columns.len();
        let subrows = kernel.subrows;
        for band in [0..radius, shape.height - radius..shape.height] {
            let sources = band.start.max(row - reach)..band.end.min(row + reach + 1);
            for source in sources {
                let slot = self.ensure(source);
                for slab in 0..subrows {
                    let weight = kernel.slab(row - source, slab);
                    let start = (slot * subrows + slab) * stored;
                    let profile = &self.profiles[start..start + stored];
                    for (value, horizontal) in self.values.iter_mut().zip(profile) {
                        *value += weight * horizontal;
                    }
                }
            }
        }
    }

    /// Profiles arc row `row` into its slot unless the slot already holds it.
    fn ensure(&mut self, row: i64) -> usize {
        let slots = i64::try_from(self.tags.len()).expect("invariant: the ring is small");
        let slot = index(self.shape.arc_ordinal(row).rem_euclid(slots));
        if self.tags[slot] == Some(row) {
            return slot;
        }
        let stored = self.columns.len();
        let subrows = self.kernel.subrows;
        let height = small_float(i64::try_from(subrows).expect("invariant: at most 16 slabs"));
        for slab in 0..subrows {
            let offset = small_float(i64::try_from(slab).expect("invariant: at most 16 slabs"));
            // The slab centre, which for sixteen slabs is exactly the fill's
            // coverage sample.
            let centre = small_float(row) + (offset + 0.5) / height;
            let (low, high) = self.shape.outline.extent_at(centre);
            let start = (slot * subrows + slab) * stored;
            for (profile, &column) in self.profiles[start..start + stored]
                .iter_mut()
                .zip(&self.columns)
            {
                *profile = self.kernel.profile(low, high, small_float(column));
            }
        }
        self.tags[slot] = Some(row);
        slot
    }

    /// Value of the last evaluated row at a column outside the interior.
    pub(super) fn edge_value(&self, column: i64) -> f64 {
        let position = if column < self.left.end {
            column - self.left.start
        } else {
            (self.left.end - self.left.start) + column - self.right.start
        };
        self.values[index(position)]
    }
}
