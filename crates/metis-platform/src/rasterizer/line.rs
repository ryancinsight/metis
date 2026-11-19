//! Ordered, discrete line geometry shared by drawing and collision queries.

use std::ops::RangeInclusive;

use super::clip_line;
use crate::framebuffer::Rect;

/// The ordered pixels of one Bresenham line segment.
///
/// Endpoint order is significant because midpoint ties round toward the
/// segment's second endpoint. Queries use the same discrete path as drawing;
/// they never clip again to the query rectangle and therefore preserve the
/// established raster phase.
///
/// # Examples
///
/// ```
/// use metis_platform::{RasterLine, Rect};
///
/// let line = RasterLine::new((0, 0), (4, 2));
/// assert!(line.intersects(Rect::new(1, 1, 1, 1)));
/// assert!(!line.intersects(Rect::new(1, 0, 1, 1)));
/// assert_eq!(line.endpoints(), ((0, 0), (4, 2)));
/// let clipped = RasterLine::clipped((-2, 5), (4, 5), Rect::new(0, 5, 3, 1));
/// assert_eq!(clipped.map(RasterLine::endpoints), Some(((0, 5), (2, 5))));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterLine {
    start: (i32, i32),
    end: (i32, i32),
}

impl RasterLine {
    /// Constructs the complete discrete path between two endpoints.
    #[must_use]
    pub const fn new(start: (i32, i32), end: (i32, i32)) -> Self {
        Self { start, end }
    }

    /// Constructs the discrete path after clipping once to `frame_bounds`.
    ///
    /// The returned path is the one [`super::draw_line`] paints when the frame
    /// has these bounds. Empty bounds or a segment outside the bounds return
    /// `None`.
    #[must_use]
    pub fn clipped(start: (i32, i32), end: (i32, i32), frame_bounds: Rect) -> Option<Self> {
        if frame_bounds.width <= 0 || frame_bounds.height <= 0 {
            return None;
        }
        let translate = |point: (i32, i32)| {
            (
                i64::from(point.0) - i64::from(frame_bounds.x),
                i64::from(point.1) - i64::from(frame_bounds.y),
            )
        };
        let (x0, y0) = translate(start);
        let (x1, y1) = translate(end);
        let (x0, y0, x1, y1) = clip_line(
            x0,
            y0,
            x1,
            y1,
            i64::from(frame_bounds.width) - 1,
            i64::from(frame_bounds.height) - 1,
        )?;
        let restore = |x: i64, y: i64| {
            Some((
                i32::try_from(x + i64::from(frame_bounds.x)).ok()?,
                i32::try_from(y + i64::from(frame_bounds.y)).ok()?,
            ))
        };
        Some(Self::new(restore(x0, y0)?, restore(x1, y1)?))
    }

    /// Returns the inclusive columns painted within an inclusive row band.
    ///
    /// `None` means the path paints no pixel in the band. The lookup is
    /// logarithmic in coordinate range: monotonic path coordinates permit two
    /// binary searches, each requiring at most 33 comparisons for `i32`
    /// endpoints.
    #[must_use]
    fn occupied_columns(self, rows: RangeInclusive<i64>) -> Option<RangeInclusive<i32>> {
        let (first, last) = self.indices_in_row_band(*rows.start(), *rows.end())?;
        let first_x = self.point_at(first).0;
        let last_x = self.point_at(last).0;
        let low = i32::try_from(first_x.min(last_x))
            .expect("invariant: a raster coordinate remains in i32");
        let high = i32::try_from(first_x.max(last_x))
            .expect("invariant: a raster coordinate remains in i32");
        Some(low..=high)
    }

    /// Tests whether this discrete path paints a pixel in a half-open rectangle.
    #[must_use]
    pub fn intersects(self, rect: Rect) -> bool {
        if rect.width <= 0 || rect.height <= 0 {
            return false;
        }
        let last_y = (i64::from(rect.y) + i64::from(rect.height) - 1).min(i64::from(i32::MAX));
        let Some(columns) = self.occupied_columns(i64::from(rect.y)..=last_y) else {
            return false;
        };
        let last_x = i64::from(rect.x) + i64::from(rect.width) - 1;
        i64::from(*columns.start()) <= last_x && i64::from(*columns.end()) >= i64::from(rect.x)
    }

    /// Returns the ordered endpoints of this fixed raster path.
    #[must_use]
    pub const fn endpoints(self) -> ((i32, i32), (i32, i32)) {
        (self.start, self.end)
    }

    pub(super) fn for_each_pixel(self, mut visit: impl FnMut(i32, i32)) {
        let (mut x, mut y) = (i64::from(self.start.0), i64::from(self.start.1));
        let (x1, y1) = (i64::from(self.end.0), i64::from(self.end.1));
        let dx = (x1 - x).abs();
        let dy = -(y1 - y).abs();
        let step_x = if x < x1 { 1 } else { -1 };
        let step_y = if y < y1 { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            visit(
                i32::try_from(x).expect("invariant: a raster coordinate remains in i32"),
                i32::try_from(y).expect("invariant: a raster coordinate remains in i32"),
            );
            if x == x1 && y == y1 {
                break;
            }
            let doubled = error * 2;
            if doubled >= dy {
                error += dy;
                x += step_x;
            }
            if doubled <= dx {
                error += dx;
                y += step_y;
            }
        }
    }

    fn indices_in_row_band(self, low: i64, high: i64) -> Option<(i64, i64)> {
        if low > high {
            return None;
        }
        let (start, end) = (self.start.1, self.end.1);
        let increasing = start <= end;
        let transformed = |index| {
            let coordinate = self.point_at(index).1;
            if increasing { coordinate } else { -coordinate }
        };
        let (band_low, band_high) = if increasing {
            (low, high)
        } else {
            (-high, -low)
        };
        let count = self.major_delta() + 1;
        let first = lower_bound(count, |index| transformed(index) >= band_low);
        let after = lower_bound(count, |index| transformed(index) > band_high);
        (first < after).then_some((first, after - 1))
    }

    fn point_at(self, index: i64) -> (i64, i64) {
        let (x0, y0) = (i64::from(self.start.0), i64::from(self.start.1));
        let (x1, y1) = (i64::from(self.end.0), i64::from(self.end.1));
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let step_x = if x0 < x1 { 1 } else { -1 };
        let step_y = if y0 < y1 { 1 } else { -1 };
        if dx >= dy {
            (
                x0 + step_x * index,
                y0 + step_y * rounded_minor(index, dy, dx),
            )
        } else {
            (
                x0 + step_x * rounded_minor(index, dx, dy),
                y0 + step_y * index,
            )
        }
    }

    fn major_delta(self) -> i64 {
        (i64::from(self.end.0) - i64::from(self.start.0))
            .abs()
            .max((i64::from(self.end.1) - i64::from(self.start.1)).abs())
    }
}

fn rounded_minor(index: i64, minor: i64, major: i64) -> i64 {
    if major == 0 {
        return 0;
    }
    let numerator = i128::from(2 * index) * i128::from(minor) + i128::from(major);
    i64::try_from(numerator / i128::from(2 * major))
        .expect("invariant: minor progress does not exceed an i32 endpoint delta")
}

fn lower_bound(count: i64, predicate: impl Fn(i64) -> bool) -> i64 {
    let (mut low, mut high) = (0, count);
    while low < high {
        let middle = low + (high - low) / 2;
        if predicate(middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}
