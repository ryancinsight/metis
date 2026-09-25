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
/// A mask is one byte per pixel of a shadow's extent clipped to the surface,
/// so any shadow on a 1600×1200 surface, at most 1,920,000 bytes, fits one
/// generation; larger surfaces can hold larger masks than a generation
/// admits, and those are rendered each time rather than retained.
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

/// A shadow's alpha over its extent clipped to the surface, row-major.
///
/// Pixels the border box covers hold zero, which leaves them untouched, and
/// each row records the one run of them the border box fills, so compositing
/// skips it: a card's interior is most of its shadow's extent.
#[derive(Debug)]
pub(super) struct ShadowMask {
    left: u32,
    top: u32,
    width: usize,
    alphas: Box<[u8]>,
    /// Per row, the mask columns `[start, end)` the border box fills.
    holes: Box<[(usize, usize)]>,
}

impl Footprint for ShadowMask {
    fn footprint(&self) -> usize {
        self.alphas.len() + self.holes.len() * size_of::<(usize, usize)>() + size_of::<Self>()
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
        let height = usize::try_from(rows.end - rows.start).expect(fits);
        let mut mask = Self {
            left: u32::try_from(columns.start).expect(fits),
            top: u32::try_from(rows.start).expect(fits),
            width,
            alphas: vec![0; width * height].into_boxed_slice(),
            holes: vec![(0, 0); height].into_boxed_slice(),
        };
        let mut field = Field::new(&kernel, &shape);
        let interior = field.interior();
        let interior = interior.start + origin_x..interior.end + origin_x;
        let mut clip = Clip {
            element,
            samples: [(0.0, 0.0); SUBSAMPLES],
            unused: [(0.0, 0.0); SUBSAMPLES],
        };
        let alpha = shadow.color().a;
        // Tiling bounds the field's scratch memory by the tile, not the width.
        let tiles = std::iter::successors(Some(columns.start), |start| Some(start + TILE_WIDTH))
            .take_while(|start| *start < columns.end)
            .map(|start| start..(start + TILE_WIDTH).min(columns.end));
        for tile in tiles {
            field.set_tile(tile.start - origin_x..tile.end - origin_x);
            let mask_rows = mask
                .alphas
                .chunks_exact_mut(width)
                .zip(mask.holes.iter_mut());
            for (device_row, (alphas, hole)) in rows.clone().zip(mask_rows) {
                render_row(
                    &mut field,
                    &mut clip,
                    alphas,
                    hole,
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
        Some(mask)
    }

    /// Composites the mask's visible part in `color`, whose alpha the mask
    /// already carries.
    pub(super) fn composite(&self, fb: &mut Framebuffer, color: Color) {
        let clip = fb.clip();
        let height = self.alphas.len() / self.width.max(1);
        let fits = "invariant: mask extents fit the surface's u32 coordinates";
        let right = self.left + u32::try_from(self.width).expect(fits);
        let bottom = self.top + u32::try_from(height).expect(fits);
        let (left, top) = (self.left.max(clip.left()), self.top.max(clip.top()));
        let (right, bottom) = (right.min(clip.right()), bottom.min(clip.bottom()));
        if left >= right || top >= bottom {
            return;
        }
        let from = usize::try_from(left - self.left).expect(fits);
        let to = usize::try_from(right - self.left).expect(fits);
        for row in top..bottom {
            let index = usize::try_from(row - self.top).expect(fits);
            let alphas = &self.alphas[index * self.width..(index + 1) * self.width];
            let (hole_start, hole_end) = self.holes[index];
            // The visible columns less the hole: at most two runs.
            for (start, end) in [(from, to.min(hole_start)), (from.max(hole_end), to)] {
                if start < end {
                    let column = self.left + u32::try_from(start).expect(fits);
                    fb.composite_alpha_row(row, column, &alphas[start..end], color);
                }
            }
        }
    }
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

/// Renders one row of one tile, leaving pixels the border box covers zero.
fn render_row(
    field: &mut Field<'_>,
    clip: &mut Clip,
    alphas: &mut [u8],
    hole: &mut (usize, usize),
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
    let index =
        |column: i64| usize::try_from(column - left).expect("invariant: columns lie in the mask");
    let mut column = columns.start;
    while column < columns.end {
        let covered = bounds.map_or(Coverage::Outside, |bounds| bounds.classify(column));
        match covered {
            Coverage::Solid(end) => {
                let end = end.min(columns.end);
                // A tile boundary can split the hole; the pieces are adjacent.
                *hole = if hole.0 == hole.1 {
                    (index(column), index(end))
                } else {
                    (hole.0.min(index(column)), hole.1.max(index(end)))
                };
                column = end;
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
                alphas[index(column)..index(end)].fill(coverage_alpha(alpha, interior_value));
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
