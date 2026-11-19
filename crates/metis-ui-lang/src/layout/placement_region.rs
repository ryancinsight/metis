//! Finite text-anchor regions compiled from fixed presentation obstacles.

use crate::parser::limit_error;
use metis_core::error::Result;
use metis_platform::{RasterLine, Rect};

mod count;

/// Fixed geometry that text glyphs must not cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlacementObstacle {
    /// A half-open axis-aligned rectangle.
    Rect(Rect),
    /// A discrete line clipped once to its render frame.
    Line(RasterLine),
}

/// Clear text anchors for one measured glyph rectangle and fixed obstacle set.
///
/// This immutable region represents anchors whose measured glyph rectangle fits `bounds`, has a
/// representable glyph origin and text anchor, and covers no supplied occupied pixel. `extent.x`
/// and `extent.y` are the bearing from anchor to glyph bounds. [`Self::contains`] checks without
/// clamping. Nonpositive or oversized geometry and an empty representability intersection produce
/// an empty region. Construction owns compiled geometry; later input-slice changes do not alter it.
///
/// For `n` obstacles, construction and membership use `O(n)` work; stored
/// geometry has at most six inequalities per nonempty obstacle. No raster
/// pixel or coordinate-span enumeration occurs. Coordinate sums use `i64`,
/// and transformed half-planes use `i128`: `|A|, |B| < 2^34` and
/// `|C| < 2^70` for `i32` inputs. Admissible glyph coordinates fit `i32`, so
/// evaluating `A*x + B*y` fits `i128`.
///
/// # Examples
///
/// ```
/// use metis_platform::{RasterLine, Rect};
/// use metis_ui_lang::{PlacementObstacle, PlacementRegion};
///
/// let occupied = [PlacementObstacle::Line(RasterLine::new((2, 0), (2, 2))),
///     PlacementObstacle::Rect(Rect::new(4, 0, 2, 1))];
/// let region = PlacementRegion::new(Rect::new(0, 0, 2, 1),
///     Rect::new(0, 0, 6, 3), &occupied)?;
/// let clear = [(0, 0), (0, 1), (3, 1), (4, 1), (0, 2), (3, 2), (4, 2)];
/// let blocked = [(1, 0), (2, 0), (3, 0), (4, 0), (1, 1), (2, 1), (1, 2), (2, 2)];
/// assert_eq!(clear.map(|anchor| region.contains(anchor)), [true; 7]);
/// assert_eq!(blocked.map(|anchor| region.contains(anchor)), [false; 8]);
/// assert_eq!([(-1, 0), (5, 0), (0, -1), (0, 3)].map(|p| region.contains(p)), [false; 4]);
/// # Ok::<(), metis_core::error::MetisError>(())
/// ```
pub struct PlacementRegion {
    extent: Rect,
    bounds: Option<OriginBounds>,
    forbidden: Vec<Polygon>,
}

impl PlacementRegion {
    /// Compiles a snapshot of obstacles, ignoring empty rectangles.
    ///
    /// # Errors
    ///
    /// Returns a layout resource error when obstacle storage cannot be
    /// allocated. Empty geometry is a valid region, not an error.
    pub fn new(extent: Rect, bounds: Rect, obstacles: &[PlacementObstacle]) -> Result<Self> {
        let Some(bounds) = OriginBounds::new(extent, bounds) else {
            return Ok(Self {
                extent,
                bounds: None,
                forbidden: Vec::new(),
            });
        };
        let mut forbidden = Vec::new();
        forbidden
            .try_reserve_exact(obstacles.len())
            .map_err(|_| limit_error("Placement obstacle allocation failed"))?;
        for obstacle in obstacles {
            match *obstacle {
                PlacementObstacle::Rect(rect) => {
                    forbidden.extend(Polygon::rect(rect, extent.width, extent.height));
                }
                PlacementObstacle::Line(line) => {
                    forbidden.push(Polygon::line(line, extent.width, extent.height));
                }
            }
        }
        Ok(Self {
            extent,
            bounds: Some(bounds),
            forbidden,
        })
    }

    /// Reports whether a supplied text anchor is representable and clear.
    #[must_use]
    pub fn contains(&self, text_anchor: (i32, i32)) -> bool {
        let Some(bounds) = self.bounds else {
            return false;
        };
        let x = i64::from(text_anchor.0) + i64::from(self.extent.x);
        let y = i64::from(text_anchor.1) + i64::from(self.extent.y);
        bounds.contains(x, y) && !self.forbidden.iter().any(|region| region.contains(x, y))
    }
}

#[derive(Clone, Copy)]
struct OriginBounds {
    x: (i64, i64),
    y: (i64, i64),
}

impl OriginBounds {
    fn new(extent: Rect, bounds: Rect) -> Option<Self> {
        if extent.width <= 0
            || extent.height <= 0
            || bounds.width <= 0
            || bounds.height <= 0
            || bounds.width < extent.width
            || bounds.height < extent.height
        {
            return None;
        }
        let range = |start: i32, span: i32, size: i32, bearing: i32| {
            let low = i64::from(start).max(i64::from(i32::MIN) + i64::from(bearing));
            let high = (i64::from(start) + i64::from(span) - i64::from(size))
                .min(i64::from(i32::MAX))
                .min(i64::from(i32::MAX) + i64::from(bearing));
            (low <= high).then_some((low, high))
        };
        Some(Self {
            x: range(bounds.x, bounds.width, extent.width, extent.x)?,
            y: range(bounds.y, bounds.height, extent.height, extent.y)?,
        })
    }

    fn contains(self, x: i64, y: i64) -> bool {
        (self.x.0..=self.x.1).contains(&x) && (self.y.0..=self.y.1).contains(&y)
    }
}

#[derive(Clone, Copy, Default)]
struct Plane(i128, i128, i128);

#[derive(Clone, Copy)]
struct Polygon {
    planes: [Plane; 6],
    len: usize,
}

impl Polygon {
    fn rect(rect: Rect, width: i32, height: i32) -> Option<Self> {
        if rect.width <= 0 || rect.height <= 0 {
            return None;
        }
        let (x, y) = (i128::from(rect.x), i128::from(rect.y));
        Some(Self::from(
            &[
                Plane(-1, 0, -x + i128::from(width) - 1),
                Plane(1, 0, x + i128::from(rect.width) - 1),
                Plane(0, -1, -y + i128::from(height) - 1),
                Plane(0, 1, y + i128::from(rect.height) - 1),
                Plane::default(),
                Plane::default(),
            ],
            4,
        ))
    }

    fn line(line: RasterLine, width: i32, height: i32) -> Self {
        let (start, end) = line.endpoints();
        let sx = if end.0 >= start.0 { 1 } else { -1 };
        let sy = if end.1 >= start.1 { 1 } else { -1 };
        let mut x = axis_transform(sx, width, (1, 0));
        let mut y = axis_transform(sy, height, (0, 1));
        let (mut x0, mut x1) = (
            i128::from(sx) * i128::from(start.0),
            i128::from(sx) * i128::from(end.0),
        );
        let (mut y0, mut y1) = (
            i128::from(sy) * i128::from(start.1),
            i128::from(sy) * i128::from(end.1),
        );
        let (mut w, mut h) = (i128::from(width), i128::from(height));
        if x1 - x0 < y1 - y0 {
            std::mem::swap(&mut x, &mut y);
            std::mem::swap(&mut x0, &mut y0);
            std::mem::swap(&mut x1, &mut y1);
            std::mem::swap(&mut w, &mut h);
        }
        let (major, minor) = (x1 - x0, y1 - y0);
        // Endpoint reflection has no glyph-size shift; reflected origins use
        // `1 - extent`. The two sloped planes encode midpoint tie direction.
        let normalized = [
            Plane(-1, 0, -x0 + w - 1),
            Plane(1, 0, x1),
            Plane(0, -1, -y0 + h - 1),
            Plane(0, 1, y1),
            Plane(
                2 * minor,
                -2 * major,
                2 * minor * x0 + 2 * major * (h - y0) - major - 1,
            ),
            Plane(
                -2 * minor,
                2 * major,
                2 * minor * (w - 1 - x0) + major + 2 * major * y0,
            ),
        ];
        let mut planes = [Plane::default(); 6];
        for (target, source) in planes.iter_mut().zip(normalized) {
            *target = restore(source, x, y);
        }
        Self::from(&planes, if major == 0 { 4 } else { 6 })
    }

    const fn from(planes: &[Plane; 6], len: usize) -> Self {
        Self { planes: *planes, len }
    }

    fn contains(self, x: i64, y: i64) -> bool {
        self.planes
            .iter()
            .take(self.len)
            .all(|plane| plane.0 * i128::from(x) + plane.1 * i128::from(y) <= plane.2)
    }
}

type Transform = (i128, i128, i128);

fn axis_transform(sign: i32, extent: i32, axis: (i32, i32)) -> Transform {
    let (x, y) = (sign * axis.0, sign * axis.1);
    (
        i128::from(x),
        i128::from(y),
        if sign > 0 { 0 } else { 1 - i128::from(extent) },
    )
}

fn restore(plane: Plane, x: Transform, y: Transform) -> Plane {
    Plane(
        plane.0 * x.0 + plane.1 * y.0,
        plane.0 * x.1 + plane.1 * y.1,
        plane.2 - plane.0 * x.2 - plane.1 * y.2,
    )
}

#[cfg(test)]
mod tests;
