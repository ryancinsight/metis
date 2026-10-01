//! A shadow's per-pixel alpha: rendered once from the blurred field, then
//! composited by every repaint that keeps the shadow's geometry.
//!
//! The field is where a shadow costs its time, and a repaint — a theme
//! change, a damage-limited repaint through the shadow — redraws the same
//! shadow at the same place. The key holds everything that determines the
//! alpha: the border box's device position and size, the clamped radius, the
//! offsets, the blur, the color's alpha and the surface size. Position is
//! part of it because arc extents are computed in absolute device
//! coordinates, so a translated shadow can round differently. A hit therefore
//! composites exactly the bytes a fresh render would produce. Eviction is the
//! [`crate::memo`] generations'.

use std::mem::size_of;
use std::ops::Range;

use super::TILE_WIDTH;
use super::field::{Field, Shape};
use super::kernel::{Kernel, small_float};
use crate::framebuffer::{Color, Framebuffer, Rect, coverage_alpha};
use crate::memo::{Footprint, GenerationalMemo};
use crate::rasterizer::BoxShadow;
use crate::rasterizer::round_rect::{
    CornerRadius, RoundRect, RowBounds, RowSamples, SUBSAMPLES, pixel_coverage, sample_row,
};

/// Mask bytes admitted to one generation.
///
/// A mask retains one byte per pixel of a shadow's extent clipped to the
/// surface, less the pixels its border box covers. A card's extent is mostly
/// that interior, so the retained bytes scale with the blur ring around it,
/// its perimeter times about twice the blur reach, not with the card's area: a
/// 3000×1800 card at blur 10 keeps a few hundred KiB where its extent is over
/// 5 MB. A mask larger than a generation, such as one with a very large blur,
/// is rendered each time rather than retained.
pub(super) const GENERATION_BYTES: usize = 4 * 1024 * 1024;

/// Everything that determines a shadow's per-pixel alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct ShadowKey {
    border_box: (i32, i32, i32, i32),
    radius: i32,
    offset: (i32, i32),
    blur: u32,
    alpha: u8,
    surface: (u32, u32),
}

impl ShadowKey {
    pub(super) const fn new(
        border_box: Rect,
        radius: CornerRadius,
        shadow: BoxShadow,
        surface: (u32, u32),
    ) -> Self {
        Self {
            border_box: (
                border_box.x,
                border_box.y,
                border_box.width,
                border_box.height,
            ),
            radius: radius.pixels(),
            offset: (shadow.offset_x(), shadow.offset_y()),
            blur: shadow.blur(),
            alpha: shadow.color().a,
            surface,
        }
    }
}

/// Shadow masks for the current and previous generations.
pub(super) type ShadowMasks = GenerationalMemo<ShadowKey, ShadowMask, GENERATION_BYTES>;

/// A shadow's alpha over its extent clipped to the surface, row by row.
///
/// Each row records the one run of columns the border box fills, which
/// compositing leaves untouched, and stores only the alphas on either side of
/// it: a card's interior is most of its shadow's extent.
#[derive(Debug)]
pub(super) struct ShadowMask {
    left: u32,
    top: u32,
    /// Columns of the extent, hole included.
    width: usize,
    /// Each row's alphas left of its hole, then right of it; rows follow one
    /// another in order.
    alphas: Box<[u8]>,
    rows: Box<[MaskRow]>,
}

/// Where one mask row keeps its alphas and which columns it omits.
#[derive(Debug, Clone, Copy)]
struct MaskRow {
    /// Index in [`ShadowMask::alphas`] of the row's first stored alpha.
    offset: usize,
    /// The mask columns `[start, end)` the border box fills; empty when it
    /// fills none.
    hole: (u32, u32),
}

impl MaskRow {
    fn hole(self) -> (usize, usize) {
        let fits = "invariant: mask columns fit usize";
        (
            usize::try_from(self.hole.0).expect(fits),
            usize::try_from(self.hole.1).expect(fits),
        )
    }
}

impl Footprint for ShadowMask {
    fn footprint(&self) -> usize {
        self.alphas.len() + self.rows.len() * size_of::<MaskRow>() + size_of::<Self>()
    }
}

#[cfg(test)]
thread_local! {
    /// Masks rendered on this thread, so a test can tell a hit from a miss.
    pub(super) static RENDERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl ShadowMask {
    /// Renders the alpha `shadow` casts from `border_box` over the part of
    /// its extent on a `surface`-sized framebuffer, or `None` when no part
    /// of it is on the surface.
    ///
    /// Each alpha is [`coverage_alpha`] of the blurred field times the share
    /// of the pixel the border box leaves uncovered, which is exactly the
    /// alpha compositing that coverage would use.
    pub(super) fn render(
        surface: (u32, u32),
        border_box: Rect,
        radius: CornerRadius,
        shadow: BoxShadow,
    ) -> Option<Self> {
        #[cfg(test)]
        RENDERS.set(RENDERS.get() + 1);
        let (Some(outline), Some(element)) = (
            RoundRect::new(Rect::new(0, 0, border_box.width, border_box.height), radius),
            RoundRect::new(border_box, radius),
        ) else {
            return None;
        };
        let shape = Shape {
            width: i64::from(border_box.width),
            height: i64::from(border_box.height),
            radius: i64::from(radius.pixels()),
            outline,
        };
        let kernel = Kernel::new(shadow.blur());
        let reach = kernel.reach;
        let origin_x = i64::from(border_box.x) + i64::from(shadow.offset_x());
        let origin_y = i64::from(border_box.y) + i64::from(shadow.offset_y());
        let visible = |start: i64, extent: i64, high: u32| {
            (start - reach).clamp(0, i64::from(high))
                ..(start + extent + reach).clamp(0, i64::from(high))
        };
        let columns = visible(origin_x, shape.width, surface.0);
        let rows = visible(origin_y, shape.height, surface.1);
        if columns.is_empty() || rows.is_empty() {
            return None;
        }
        let fits = "invariant: surface coordinates fit u32 and usize";
        let width = usize::try_from(columns.end - columns.start).expect(fits);
        let mut clip = Clip {
            element,
            samples: [(0.0, 0.0); SUBSAMPLES],
            unused: [(0.0, 0.0); SUBSAMPLES],
        };
        let (mask_rows, stored) = layout_rows(&mut clip, rows.clone(), &columns);
        let mut alphas = vec![0; stored].into_boxed_slice();
        let mut row_alphas = split_rows(&mut alphas, &mask_rows, width);
        let mut field = Field::new(&kernel, &shape);
        let interior = field.interior();
        let interior = interior.start + origin_x..interior.end + origin_x;
        let alpha = shadow.color().a;
        // Tiling bounds the field's scratch memory by the tile, not the width.
        let tiles = std::iter::successors(Some(columns.start), |start| Some(start + TILE_WIDTH))
            .take_while(|start| *start < columns.end)
            .map(|start| start..(start + TILE_WIDTH).min(columns.end));
        for tile in tiles {
            field.set_tile(tile.start - origin_x..tile.end - origin_x);
            let pieces = row_alphas.iter_mut().zip(mask_rows.iter());
            for (device_row, (alphas, row)) in rows.clone().zip(pieces) {
                render_row(
                    &mut field,
                    &mut clip,
                    alphas,
                    row.hole(),
                    RowSpan {
                        row: device_row,
                        columns: tile.clone(),
                        left: columns.start,
                        origin: (origin_x, origin_y),
                        interior: interior.clone(),
                    },
                    alpha,
                );
            }
        }
        Some(Self {
            left: u32::try_from(columns.start).expect(fits),
            top: u32::try_from(rows.start).expect(fits),
            width,
            alphas,
            rows: mask_rows,
        })
    }

    /// Composites the mask's visible part in `color`, whose alpha the mask
    /// already carries.
    pub(super) fn composite(&self, fb: &mut Framebuffer, color: Color) {
        let clip = fb.clip();
        let fits = "invariant: mask extents fit the surface's u32 coordinates";
        let right = self.left + u32::try_from(self.width).expect(fits);
        let bottom = self.top + u32::try_from(self.rows.len()).expect(fits);
        let (left, top) = (self.left.max(clip.left()), self.top.max(clip.top()));
        let (right, bottom) = (right.min(clip.right()), bottom.min(clip.bottom()));
        if left >= right || top >= bottom {
            return;
        }
        let from = usize::try_from(left - self.left).expect(fits);
        let to = usize::try_from(right - self.left).expect(fits);
        for row in top..bottom {
            let stored = self.rows[usize::try_from(row - self.top).expect(fits)];
            let (hole_start, hole_end) = stored.hole();
            // The visible columns less the hole: at most two runs. A column
            // left of the hole is stored at the row's offset plus itself, one
            // right of it after the stored left run.
            for (start, end) in [(from, to.min(hole_start)), (from.max(hole_end), to)] {
                if start < end {
                    let first = if start >= hole_end {
                        stored.offset + hole_start + (start - hole_end)
                    } else {
                        stored.offset + start
                    };
                    let alphas = &self.alphas[first..first + (end - start)];
                    let column = self.left + u32::try_from(start).expect(fits);
                    fb.composite_alpha_row(row, column, alphas, color);
                }
            }
        }
    }
}

/// Places each of `rows` in the mask: the columns of `columns` its border box
/// fills, and where its remaining alphas start. Also returns the stored total.
fn layout_rows(clip: &mut Clip, rows: Range<i64>, columns: &Range<i64>) -> (Box<[MaskRow]>, usize) {
    let width = usize::try_from(columns.end - columns.start).expect("invariant: fits usize");
    let column = |value: i64| {
        u32::try_from(value - columns.start).expect("invariant: columns lie in the mask")
    };
    let mut stored = 0;
    let layout = rows
        .map(|device_row| {
            let surface_row =
                u32::try_from(device_row).expect("invariant: rows are clamped to the surface");
            let hole = clip.row(surface_row).map_or((0, 0), |bounds| {
                let start = bounds.solid.0.max(columns.start);
                let end = bounds.solid.1.min(columns.end);
                if start < end {
                    (column(start), column(end))
                } else {
                    (0, 0)
                }
            });
            let row = MaskRow {
                offset: stored,
                hole,
            };
            let (start, end) = row.hole();
            stored += width - (end - start);
            row
        })
        .collect();
    (layout, stored)
}

/// Splits `alphas` into the stored alphas of each of `rows`, in order.
fn split_rows<'alphas>(
    alphas: &'alphas mut [u8],
    rows: &[MaskRow],
    width: usize,
) -> Vec<&'alphas mut [u8]> {
    let mut rest = alphas;
    rows.iter()
        .map(|row| {
            let (start, end) = row.hole();
            let (piece, tail) = std::mem::take(&mut rest).split_at_mut(width - (end - start));
            rest = tail;
            piece
        })
        .collect()
}

/// One device row of one tile, with the mask's and the shadow's placement.
struct RowSpan {
    row: i64,
    columns: Range<i64>,
    /// Device column of the mask row's first alpha.
    left: i64,
    origin: (i64, i64),
    /// Device columns whose value is the row's interior value.
    interior: Range<i64>,
}

/// Renders one row of one tile into `alphas`, the row's stored alphas, which
/// omit the `hole` columns the border box fills.
fn render_row(
    field: &mut Field<'_>,
    clip: &mut Clip,
    alphas: &mut [u8],
    hole: (usize, usize),
    span: RowSpan,
    alpha: u8,
) {
    let RowSpan {
        row: device_row,
        columns,
        left,
        origin: (origin_x, origin_y),
        interior,
    } = span;
    let row = device_row - origin_y;
    field.evaluate(row);
    let interior_value = field.interior_value(row);
    let surface_row =
        u32::try_from(device_row).expect("invariant: rows are clamped to the surface");
    let bounds = clip.row(surface_row);
    let hole_len = hole.1 - hole.0;
    // A stored alpha's index, for a column that is not in the hole.
    let index = |column: i64| {
        let column = usize::try_from(column - left).expect("invariant: columns lie in the mask");
        if column < hole.0 {
            column
        } else {
            column - hole_len
        }
    };
    // The index one past the last stored alpha of a run that ends at `column`,
    // which may be the hole's first column.
    let run_end = |column: i64| {
        let column = usize::try_from(column - left).expect("invariant: columns lie in the mask");
        if column <= hole.0 {
            column
        } else {
            column - hole_len
        }
    };
    let mut column = columns.start;
    while column < columns.end {
        let covered = bounds.map_or(Coverage::Outside, |bounds| bounds.classify(column));
        match covered {
            Coverage::Solid(end) => {
                // The hole's columns have no stored alpha.
                column = end.min(columns.end);
                continue;
            }
            Coverage::Outside if interior.contains(&column) => {
                // The interior value is constant along the row, so the run up
                // to the next border-box pixel takes one alpha.
                let mut end = interior.end.min(columns.end);
                if let Some(bounds) = bounds
                    && bounds.touched.0 > column
                {
                    end = end.min(bounds.touched.0);
                }
                alphas[index(column)..run_end(end)].fill(coverage_alpha(alpha, interior_value));
                column = end;
                continue;
            }
            Coverage::Outside | Coverage::Partial => {}
        }
        let value = if interior.contains(&column) {
            interior_value
        } else {
            field.edge_value(column - origin_x)
        };
        let uncovered = match covered {
            Coverage::Partial => 1.0 - pixel_coverage(&clip.samples, None, small_float(column)),
            _ => 1.0,
        };
        alphas[index(column)] = coverage_alpha(alpha, value * uncovered);
        column += 1;
    }
}

/// How the border box covers one device pixel.
#[derive(Clone, Copy)]
enum Coverage {
    /// Not at all.
    Outside,
    /// Partly, along an arc.
    Partial,
    /// Completely, through the returned exclusive end column.
    Solid(i64),
}

/// Border-box samples for the clip, reused across rows.
struct Clip {
    element: RoundRect,
    samples: RowSamples,
    unused: RowSamples,
}

/// Device columns the border box reaches and fills on one row.
#[derive(Clone, Copy)]
struct ClipRow {
    touched: (i64, i64),
    solid: (i64, i64),
}

impl Clip {
    fn row(&mut self, row: u32) -> Option<ClipRow> {
        let RowBounds { touched, solid, .. } =
            sample_row(row, self.element, None, &mut self.samples, &mut self.unused);
        if touched.1 <= touched.0 {
            return None;
        }
        let column = |value: f64| {
            // Border-box bounds are i32 coordinates, so rounding them to whole
            // columns stays inside i64.
            #[expect(
                clippy::cast_possible_truncation,
                reason = "border-box columns are integral values inside the i32 range"
            )]
            let column = value as i64;
            column
        };
        let solid_start = column(solid.0.ceil());
        Some(ClipRow {
            touched: (column(touched.0.floor()), column(touched.1.ceil())),
            solid: (solid_start, column(solid.1.floor()).max(solid_start)),
        })
    }
}

impl ClipRow {
    fn classify(self, column: i64) -> Coverage {
        if column >= self.solid.0 && column < self.solid.1 {
            Coverage::Solid(self.solid.1)
        } else if column >= self.touched.0 && column < self.touched.1 {
            Coverage::Partial
        } else {
            Coverage::Outside
        }
    }
}

#[cfg(test)]
#[path = "mask_tests.rs"]
mod tests;
