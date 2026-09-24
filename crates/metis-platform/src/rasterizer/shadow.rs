//! Gaussian-blurred outer box shadows.
//!
//! CSS Backgrounds and Borders Level 3 §6.1.2 defines the blur as a Gaussian
//! whose standard deviation is half the blur radius and accepts any image
//! whose pixels lie within 5% of that result. §6.1.1 casts the outer shadow
//! from the border box, offset but otherwise the same size and shape (this
//! subset has no spread distance), and clips it inside the border box.
//!
//! The Gaussian is separable. Rows outside the corner bands span the full
//! width, so both passes over them reduce to differences of the running sum of
//! a kernel whose taps are the Gaussian's mass over each pixel; for a
//! pixel-aligned edge that is exactly the continuous blur sampled at the pixel
//! centre. Rows crossing an arc are integrated as slabs with an exact profile
//! along x and the Gaussian's exact mass along y ([`field`]). Arc rows are kept
//! in a ring and columns are processed in tiles, so memory is bounded by the
//! blur and the tile rather than the radius, the shape or the surface.

mod field;
mod kernel;

use super::round_rect::{
    CornerRadius, RoundRect, RowBounds, RowSamples, SUBSAMPLES, composite_pixel, pixel_coverage,
    sample_row,
};
use std::ops::Range;

use crate::framebuffer::{Color, Framebuffer, Rect, SourceOver};
use field::{Field, Shape};
use kernel::{Kernel, small_float};

/// Visible columns evaluated together.
///
/// The arc-row ring holds `min(2K + 1, 2r) · S` rows of one tile. Its largest
/// product is `2K + 1 = 897` at the maximum blur, where `S = 1`, so a tile
/// bounds the ring at `897 · 1024 · 8` bytes, 7.3 MB, whatever the surface
/// width.
const TILE_WIDTH: i64 = 1024;

/// An outer shadow cast by a border box.
///
/// # Examples
///
/// ```
/// use metis_platform::framebuffer::Color;
/// use metis_platform::rasterizer::BoxShadow;
///
/// let shadow = BoxShadow::new(0, 4, 12, Color::rgba(0, 0, 0, 60)).expect("blur in range");
/// assert_eq!(shadow.blur(), 12);
/// assert!(BoxShadow::new(0, 0, BoxShadow::MAX_BLUR + 1, Color::BLACK).is_none());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxShadow {
    offset_x: i32,
    offset_y: i32,
    blur: u32,
    color: Color,
}

impl BoxShadow {
    /// Largest blur radius in device pixels.
    ///
    /// Work and scratch memory grow with the kernel reach, `1.75` pixels per
    /// blur pixel, so the bound caps both: at this radius the row ring holds
    /// 897 rows of the visible shadow width.
    pub const MAX_BLUR: u32 = 256;

    /// Describes a shadow, or `None` when `blur` exceeds [`Self::MAX_BLUR`].
    #[must_use]
    pub const fn new(offset_x: i32, offset_y: i32, blur: u32, color: Color) -> Option<Self> {
        if blur > Self::MAX_BLUR {
            return None;
        }
        Some(Self {
            offset_x,
            offset_y,
            blur,
            color,
        })
    }

    /// Horizontal offset of the shadow from the border box.
    #[must_use]
    pub const fn offset_x(self) -> i32 {
        self.offset_x
    }

    /// Vertical offset of the shadow from the border box.
    #[must_use]
    pub const fn offset_y(self) -> i32 {
        self.offset_y
    }

    /// Blur radius in device pixels; twice the Gaussian's standard deviation.
    #[must_use]
    pub const fn blur(self) -> u32 {
        self.blur
    }

    /// Straight RGBA shadow color.
    #[must_use]
    pub const fn color(self) -> Color {
        self.color
    }

    /// The device pixels [`draw_box_shadow`] can change for `border_box`:
    /// the offset box widened by the blur's reach on every side.
    ///
    /// # Panics
    ///
    /// Does not panic: the reach of a blur at most [`Self::MAX_BLUR`] fits
    /// `i32`.
    #[must_use]
    pub fn extent(self, border_box: Rect) -> Rect {
        let reach = i32::try_from(kernel::reach(self.blur))
            .expect("invariant: the reach of a bounded blur fits i32");
        Rect::new(
            border_box
                .x
                .saturating_add(self.offset_x)
                .saturating_sub(reach),
            border_box
                .y
                .saturating_add(self.offset_y)
                .saturating_sub(reach),
            border_box.width.saturating_add(reach.saturating_mul(2)),
            border_box.height.saturating_add(reach.saturating_mul(2)),
        )
    }
}

/// Paints an outer box shadow for `border_box` beneath its element.
///
/// The shadow is the border box offset by the shadow's offsets, rounded by
/// the same clamped radius and blurred by a Gaussian of standard deviation
/// `blur / 2`; pixels the border box covers are left untouched and pixels its
/// arcs cover partly receive the uncovered share. A zero blur paints the
/// antialiased offset shape exactly as [`super::fill_rect`] would.
///
/// # Panics
///
/// Does not panic for a valid [`Framebuffer`]; the conversion checks encode
/// the clipping invariant that every painted row and column lies on the
/// surface and every shape offset fits the `i32` extents it came from.
pub fn draw_box_shadow(
    fb: &mut Framebuffer,
    border_box: Rect,
    radius: CornerRadius,
    shadow: BoxShadow,
) {
    let color = shadow.color;
    if color.a == 0 || border_box.width <= 0 || border_box.height <= 0 {
        return;
    }
    let radius = CornerRadius::clamped(radius.pixels(), border_box);
    let (Some(outline), Some(element)) = (
        RoundRect::new(Rect::new(0, 0, border_box.width, border_box.height), radius),
        RoundRect::new(border_box, radius),
    ) else {
        return;
    };
    let shape = Shape {
        width: i64::from(border_box.width),
        height: i64::from(border_box.height),
        radius: i64::from(radius.pixels()),
        outline,
    };
    let kernel = Kernel::new(shadow.blur);
    let reach = kernel.reach;
    let origin_x = i64::from(border_box.x) + i64::from(shadow.offset_x);
    let origin_y = i64::from(border_box.y) + i64::from(shadow.offset_y);
    let clip = fb.clip();
    let visible = |start: i64, extent: i64, low: u32, high: u32| {
        let (low, high) = (i64::from(low), i64::from(high));
        (start - reach).clamp(low, high)..(start + extent + reach).clamp(low, high)
    };
    let columns = visible(origin_x, shape.width, clip.left(), clip.right());
    let rows = visible(origin_y, shape.height, clip.top(), clip.bottom());
    if columns.is_empty() || rows.is_empty() {
        return;
    }
    let mut field = Field::new(&kernel, &shape);
    let interior = field.interior();
    let interior = interior.start + origin_x..interior.end + origin_x;
    let mut clip = Clip {
        element,
        samples: [(0.0, 0.0); SUBSAMPLES],
        unused: [(0.0, 0.0); SUBSAMPLES],
    };
    // Tiling bounds the scratch memory by the tile, not the surface width.
    let tiles = std::iter::successors(Some(columns.start), |start| Some(start + TILE_WIDTH))
        .take_while(|start| *start < columns.end)
        .map(|start| start..(start + TILE_WIDTH).min(columns.end));
    for tile in tiles {
        field.set_tile(tile.start - origin_x..tile.end - origin_x);
        for device_row in rows.clone() {
            paint_row(
                fb,
                &mut field,
                &mut clip,
                RowSpan {
                    row: device_row,
                    columns: tile.clone(),
                    origin: (origin_x, origin_y),
                    interior: interior.clone(),
                },
                color,
            );
        }
    }
}

/// One device row of one tile, with the shadow's placement.
struct RowSpan {
    row: i64,
    columns: Range<i64>,
    origin: (i64, i64),
    /// Device columns whose value is the row's interior value.
    interior: Range<i64>,
}

/// Composites one row of one tile, skipping pixels the border box covers.
fn paint_row(
    fb: &mut Framebuffer,
    field: &mut Field<'_>,
    clip: &mut Clip,
    span: RowSpan,
    color: Color,
) {
    let RowSpan {
        row: device_row,
        columns,
        origin: (origin_x, origin_y),
        interior,
    } = span;
    {
        let row = device_row - origin_y;
        field.evaluate(row);
        let interior_value = field.interior_value(row);
        let surface_row =
            u32::try_from(device_row).expect("invariant: rows are clamped to the surface");
        let bounds = clip.row(surface_row);
        let mut column = columns.start;
        while column < columns.end {
            let covered = bounds.map_or(Coverage::Outside, |bounds| bounds.classify(column));
            match covered {
                Coverage::Solid(end) => {
                    column = end.min(columns.end);
                    continue;
                }
                Coverage::Outside if interior.contains(&column) => {
                    // The interior value is constant along the row, so the
                    // run up to the next border-box pixel composites at once.
                    let mut end = interior.end.min(columns.end);
                    if let Some(bounds) = bounds
                        && bounds.touched.0 > column
                    {
                        end = end.min(bounds.touched.0);
                    }
                    composite_run(fb, surface_row, column, end, color, interior_value);
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
            composite_pixel(
                fb,
                surface_row,
                small_float(column),
                color,
                value * uncovered,
            );
            column += 1;
        }
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

/// Composites a constant shadow value over device columns `[from, to)`.
fn composite_run(fb: &mut Framebuffer, row: u32, from: i64, to: i64, color: Color, value: f64) {
    // The value is a convolution of a coverage in [0, 1] with a kernel whose
    // mass is at most one, so it is itself a coverage.
    let source = SourceOver::covering(color, value);
    if source.is_transparent() {
        return;
    }
    let bounded = "invariant: runs are clamped to the surface";
    fb.composite_span(
        row,
        u32::try_from(from).expect(bounded),
        u32::try_from(to).expect(bounded),
        source,
    );
}

#[cfg(test)]
#[path = "shadow_tests.rs"]
mod tests;
