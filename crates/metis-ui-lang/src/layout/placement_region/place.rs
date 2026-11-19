use super::count::{Scratch, events, row_origin, slab_count};
use super::{OriginBounds, PlacementRegion, Polygon};
use crate::parser::limit_error;
use metis_core::error::Result;
use metis_platform::Rect;

impl PlacementRegion {
    /// Finds the first clear glyph rectangle in row-major proximity order.
    ///
    /// Search checks the preferred row first, then lower rows in ascending
    /// order, followed by upper rows in descending order. Within a row it
    /// chooses the first clear origin at or right of the preferred origin,
    /// otherwise the last clear origin to its left. The preferred text anchor
    /// is clamped to the representable glyph-origin domain. Exact slab counts
    /// skip blocked coordinate spans and locate a nonempty row by prefix or
    /// suffix count, so the search never enumerates rows or raster pixels.
    ///
    /// # Errors
    ///
    /// Returns a resource error on allocation or capacity failure. An empty or
    /// fully occupied region returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use metis_platform::{RasterLine, Rect};
    /// use metis_ui_lang::{PlacementObstacle, PlacementRegion};
    ///
    /// let occupied = [PlacementObstacle::Line(RasterLine::new((0, 0), (99, 99)))];
    /// let region = PlacementRegion::new(
    ///     Rect::new(0, 0, 4, 4),
    ///     Rect::new(0, 0, 100, 100),
    ///     &occupied,
    /// )?;
    /// assert_eq!(region.place((8, 8))?, Some(Rect::new(12, 8, 4, 4)));
    /// # Ok::<(), metis_core::error::MetisError>(())
    /// ```
    pub fn place(&self, preferred_text_anchor: (i32, i32)) -> Result<Option<Rect>> {
        let Some(bounds) = self.bounds else {
            return Ok(None);
        };
        let preferred_x = preferred_coordinate(preferred_text_anchor.0, self.extent.x, bounds.x);
        let preferred_y = preferred_coordinate(preferred_text_anchor.1, self.extent.y, bounds.y);
        let event_rows = events(&self.forbidden, bounds, Some(preferred_y))?;
        let mut scratch = Scratch::new(self.forbidden.len())?;
        if let Some(x) = row_origin(
            &self.forbidden,
            preferred_y,
            preferred_x,
            bounds,
            &mut scratch,
        ) {
            return Ok(Some(self.rectangle(x, preferred_y)));
        }
        let pivot = event_rows
            .binary_search(&preferred_y)
            .map_err(|_| limit_error("Placement events omitted the preferred row"))?;
        for index in pivot..event_rows.len().saturating_sub(1) {
            let first = event_rows[index] + 1;
            let last = event_rows[index + 1] - 1;
            if let Some(row) = locate_clear_row(
                &self.forbidden,
                first,
                last,
                bounds,
                &mut scratch,
                RowDirection::First,
            ) {
                let x = clear_origin(&self.forbidden, row, preferred_x, bounds, &mut scratch);
                return Ok(Some(self.rectangle(x, row)));
            }
            let row = event_rows[index + 1];
            if let Some(x) = row_origin(&self.forbidden, row, preferred_x, bounds, &mut scratch) {
                return Ok(Some(self.rectangle(x, row)));
            }
        }
        for index in (1..=pivot).rev() {
            let first = event_rows[index - 1] + 1;
            let last = event_rows[index] - 1;
            if let Some(row) = locate_clear_row(
                &self.forbidden,
                first,
                last,
                bounds,
                &mut scratch,
                RowDirection::Last,
            ) {
                let x = clear_origin(&self.forbidden, row, preferred_x, bounds, &mut scratch);
                return Ok(Some(self.rectangle(x, row)));
            }
            let row = event_rows[index - 1];
            if let Some(x) = row_origin(&self.forbidden, row, preferred_x, bounds, &mut scratch) {
                return Ok(Some(self.rectangle(x, row)));
            }
        }
        Ok(None)
    }

    fn rectangle(&self, x: i64, y: i64) -> Rect {
        Rect::new(
            i32::try_from(x).expect("invariant: a feasible x origin fits i32"),
            i32::try_from(y).expect("invariant: a feasible y origin fits i32"),
            self.extent.width,
            self.extent.height,
        )
    }
}

fn preferred_coordinate(anchor: i32, bearing: i32, bounds: (i64, i64)) -> i64 {
    (i64::from(anchor) + i64::from(bearing)).clamp(bounds.0, bounds.1)
}

#[derive(Clone, Copy)]
enum RowDirection {
    First,
    Last,
}

fn locate_clear_row(
    polygons: &[Polygon],
    mut first: i64,
    mut last: i64,
    bounds: OriginBounds,
    scratch: &mut Scratch,
    direction: RowDirection,
) -> Option<i64> {
    if slab_count(polygons, first, last, bounds, scratch) == 0 {
        return None;
    }
    let from_end = matches!(direction, RowDirection::Last);
    while first < last {
        let midpoint =
            i64::midpoint(first, last) + i64::from(from_end && ((first ^ last) & 1) != 0);
        if from_end {
            if slab_count(polygons, midpoint, last, bounds, scratch) > 0 {
                first = midpoint;
            } else {
                last = midpoint - 1;
            }
        } else if slab_count(polygons, first, midpoint, bounds, scratch) > 0 {
            last = midpoint;
        } else {
            first = midpoint + 1;
        }
    }
    Some(first)
}

fn clear_origin(
    polygons: &[Polygon],
    row: i64,
    preferred_x: i64,
    bounds: OriginBounds,
    scratch: &mut Scratch,
) -> i64 {
    row_origin(polygons, row, preferred_x, bounds, scratch)
        .expect("invariant: a counted row has a clear origin")
}
