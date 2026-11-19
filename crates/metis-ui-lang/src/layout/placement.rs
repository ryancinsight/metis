//! Per-container child-placement records and the one arena that stores them.
//!
//! The records are the layout pass's scratch: where each child's commands
//! landed and what the container has to redistribute. One arena serves the
//! whole tree, sized once from the node count.

use std::ops::Range;

use crate::parser::limit_error;
use crate::style::ComputedStyle;
use metis_core::error::Result;
use metis_platform::{DisplayScale, Rect};

/// Places a measured rectangle inside bounds without crossing caller-owned obstacles.
///
/// `extent` is relative to `origin`, including any left or top bearing. The
/// callback returns `None` when a candidate is clear or the bounds of a real
/// obstacle that intersects it. Search steps cross obstacle edges rather than
/// enumerating pixels: rows start at the requested origin, continue downward,
/// then search upward, and each row searches right before falling back left.
/// The callback must query one fixed finite obstacle set for the call and may
/// return an enclosing union of intersecting obstacles. Runtime follows the
/// returned obstacle-edge chains rather than the coordinate span. Invalid or
/// non-intersecting obstacle bounds reject the placement.
///
/// # Examples
///
/// ```
/// use metis_platform::Rect;
/// use metis_ui_lang::place_rect_avoiding;
///
/// let obstacle = Rect::new(4, 0, 4, 4);
/// let placed = place_rect_avoiding(
///     Rect::new(0, 0, 3, 3),
///     (4, 0),
///     Rect::new(0, 0, 12, 6),
///     |candidate| {
///         (candidate.x < 8 && 4 < candidate.x + candidate.width)
///             .then_some(obstacle)
///     },
/// );
/// assert_eq!(placed, Some(Rect::new(8, 0, 3, 3)));
/// ```
#[must_use]
pub fn place_rect_avoiding(
    extent: Rect,
    origin: (i32, i32),
    bounds: Rect,
    mut obstacle: impl FnMut(Rect) -> Option<Rect>,
) -> Option<Rect> {
    if extent.width <= 0
        || extent.height <= 0
        || bounds.width <= 0
        || bounds.height <= 0
        || extent.width > bounds.width
        || extent.height > bounds.height
    {
        return None;
    }
    let right = representable_end(bounds.x, bounds.width, extent.width)?;
    let bottom = representable_end(bounds.y, bounds.height, extent.height)?;
    let x = requested_coordinate(origin.0, extent.x, bounds.x, right)?;
    let y = requested_coordinate(origin.1, extent.y, bounds.y, bottom)?;

    let initial_edges = match search_row(extent, x, y, bounds.x, right, &mut obstacle) {
        RowSearch::Placed(candidate) => return Some(candidate),
        RowSearch::Blocked(edges) => edges,
        RowSearch::Invalid => return None,
    };

    let mut below = initial_edges.below;
    while let Some(row) = below.filter(|row| *row <= i64::from(bottom)) {
        let row = i32::try_from(row).ok()?;
        match search_row(extent, x, row, bounds.x, right, &mut obstacle) {
            RowSearch::Placed(candidate) => return Some(candidate),
            RowSearch::Blocked(edges) => below = edges.below,
            RowSearch::Invalid => return None,
        }
    }

    let mut above = initial_edges.above;
    while let Some(row) = above.filter(|row| *row >= i64::from(bounds.y)) {
        let row = i32::try_from(row).ok()?;
        match search_row(extent, x, row, bounds.x, right, &mut obstacle) {
            RowSearch::Placed(candidate) => return Some(candidate),
            RowSearch::Blocked(edges) => above = edges.above,
            RowSearch::Invalid => return None,
        }
    }
    None
}

fn representable_end(start: i32, span: i32, extent: i32) -> Option<i32> {
    let end = i64::from(start) + i64::from(span) - i64::from(extent);
    i32::try_from(end.min(i64::from(i32::MAX))).ok()
}

fn requested_coordinate(origin: i32, bearing: i32, low: i32, high: i32) -> Option<i32> {
    let requested = i64::from(origin) + i64::from(bearing);
    i32::try_from(requested.clamp(i64::from(low), i64::from(high))).ok()
}

#[derive(Clone, Copy)]
struct RowEdges {
    below: Option<i64>,
    above: Option<i64>,
}

impl RowEdges {
    fn record(&mut self, obstacle: Rect, extent_height: i32) {
        let below = i64::from(obstacle.y) + i64::from(obstacle.height);
        self.below = Some(self.below.map_or(below, |current| current.min(below)));
        let above = i64::from(obstacle.y) - i64::from(extent_height);
        self.above = Some(self.above.map_or(above, |current| current.max(above)));
    }
}

enum RowSearch {
    Placed(Rect),
    Blocked(RowEdges),
    Invalid,
}

fn search_row(
    extent: Rect,
    start_x: i32,
    row: i32,
    left: i32,
    right: i32,
    obstacle: &mut impl FnMut(Rect) -> Option<Rect>,
) -> RowSearch {
    let mut edges = RowEdges {
        below: None,
        above: None,
    };
    let mut x = i64::from(start_x);
    while x <= i64::from(right) {
        let Ok(candidate_x) = i32::try_from(x) else {
            return RowSearch::Invalid;
        };
        let candidate = Rect::new(candidate_x, row, extent.width, extent.height);
        let Some(blocker) = obstacle(candidate) else {
            return RowSearch::Placed(candidate);
        };
        if !rectangles_overlap(candidate, blocker) {
            return RowSearch::Invalid;
        }
        edges.record(blocker, extent.height);
        x = i64::from(blocker.x) + i64::from(blocker.width);
    }

    x = i64::from(start_x);
    while x >= i64::from(left) {
        let Ok(candidate_x) = i32::try_from(x) else {
            return RowSearch::Invalid;
        };
        let candidate = Rect::new(candidate_x, row, extent.width, extent.height);
        let Some(blocker) = obstacle(candidate) else {
            return RowSearch::Placed(candidate);
        };
        if !rectangles_overlap(candidate, blocker) {
            return RowSearch::Invalid;
        }
        edges.record(blocker, extent.height);
        x = i64::from(blocker.x) - i64::from(extent.width);
    }
    RowSearch::Blocked(edges)
}

fn rectangles_overlap(left: Rect, right: Rect) -> bool {
    if right.width <= 0 || right.height <= 0 {
        return false;
    }
    i64::from(left.x) < i64::from(right.x) + i64::from(right.width)
        && i64::from(right.x) < i64::from(left.x) + i64::from(left.width)
        && i64::from(left.y) < i64::from(right.y) + i64::from(right.height)
        && i64::from(right.y) < i64::from(left.y) + i64::from(left.height)
}

/// Where one child's commands and extents landed during child layout.
pub(super) struct ChildPlacement {
    /// First command this child emitted, in painter order.
    pub(super) first_command: usize,
    /// One past this child's last command.
    pub(super) end_command: usize,
    /// Extent along the container's cross axis.
    pub(super) cross: i32,
}

/// What child layout leaves for the container to redistribute.
pub(super) struct ChildExtent {
    /// Content height the container uses for its automatic height.
    pub(super) content_height: i32,
    /// Main-axis span the children occupied, gaps included.
    pub(super) main_used: i32,
    /// The container's own records in [`PlacementArena::items`], one per
    /// visible child, in painter order.
    pub(super) placements: Range<usize>,
}

/// Storage for the child placements of every container the current layout is
/// inside.
///
/// A container appends its records above its children's and releases them once
/// it has aligned them, so one buffer serves the whole tree instead of one per
/// container. The buffer is sized once from the node count the resource
/// validation already walks.
#[derive(Default)]
pub(super) struct PlacementArena {
    pub(super) items: Vec<ChildPlacement>,
}

impl PlacementArena {
    pub(super) fn with_capacity(nodes: usize) -> Self {
        let mut arena = Self::default();
        // The bound is the document's own node count, so this is the only
        // request a layout makes for placements. A failed reservation leaves
        // the arena empty, and `children_extent` reports the failure it hits
        // instead.
        let _ = arena.items.try_reserve_exact(nodes);
        arena
    }
}

#[derive(Clone, Copy)]
pub(super) struct ChildLayout<'style> {
    pub(super) style: &'style ComputedStyle,
    pub(super) content_x: i32,
    pub(super) content_y: i32,
    pub(super) content_width: i32,
    pub(super) available_height: i32,
    pub(super) row: bool,
    pub(super) gap: i32,
    pub(super) display_scale: DisplayScale,
}

pub(super) fn mul(left: i32, right: i32) -> Result<i32> {
    left.checked_mul(right)
        .ok_or_else(|| limit_error("Layout text extent overflow"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use metis_platform::clip_line_to_rect;

    fn text_obstacle(candidate: Rect, obstacles: &[Rect]) -> Option<Rect> {
        obstacles
            .iter()
            .copied()
            .find(|obstacle| rectangles_overlap(candidate, *obstacle))
    }

    fn line_obstacle(candidate: Rect, start: (i32, i32), end: (i32, i32)) -> Option<Rect> {
        clip_line_to_rect(start, end, candidate).map(|(start, end)| {
            let left = start.0.min(end.0);
            let top = start.1.min(end.1);
            Rect::new(
                left,
                top,
                start.0.max(end.0) - left + 1,
                start.1.max(end.1) - top + 1,
            )
        })
    }

    #[test]
    fn rectangle_placement_preserves_text_collision_search_order() {
        let bounds = Rect::new(0, 0, 80, 40);
        let extent = Rect::new(-1, -2, 12, 8);
        let right_obstacles = [Rect::new(20, 10, 20, 8), Rect::new(40, 10, 20, 8)];
        assert_eq!(
            place_rect_avoiding(extent, (22, 12), bounds, |candidate| {
                text_obstacle(candidate, &right_obstacles)
            }),
            Some(Rect::new(60, 10, 12, 8))
        );

        let spanning = [Rect::new(21, 10, 59, 8)];
        assert_eq!(
            place_rect_avoiding(extent, (22, 12), bounds, |candidate| {
                text_obstacle(candidate, &spanning)
            }),
            Some(Rect::new(9, 10, 12, 8))
        );

        let full_row = [Rect::new(0, 10, 80, 8)];
        assert_eq!(
            place_rect_avoiding(extent, (22, 12), bounds, |candidate| {
                text_obstacle(candidate, &full_row)
            }),
            Some(Rect::new(21, 18, 12, 8))
        );
    }

    #[test]
    fn rectangle_placement_accepts_clear_representable_extremes() {
        assert_eq!(
            place_rect_avoiding(
                Rect::new(0, 0, 3, 4),
                (7, 6),
                Rect::new(0, 0, 20, 20),
                |_| None,
            ),
            Some(Rect::new(7, 6, 3, 4))
        );
        assert_eq!(
            place_rect_avoiding(
                Rect::new(0, 0, 1, 1),
                (i32::MAX, 0),
                Rect::new(i32::MAX, 0, 1, 1),
                |_| None,
            ),
            Some(Rect::new(i32::MAX, 0, 1, 1))
        );
    }

    #[test]
    fn rectangle_placement_falls_back_upward_by_obstacle_edge() {
        let obstacle = Rect::new(0, 10, 20, 10);
        assert_eq!(
            place_rect_avoiding(
                Rect::new(0, 0, 4, 4),
                (5, 10),
                Rect::new(0, 0, 20, 20),
                |candidate| rectangles_overlap(candidate, obstacle).then_some(obstacle),
            ),
            Some(Rect::new(5, 6, 4, 4))
        );
    }

    #[test]
    fn rectangle_placement_rejects_fully_occupied_bounds() {
        let bounds = Rect::new(0, 0, 20, 20);
        assert_eq!(
            place_rect_avoiding(Rect::new(0, 0, 4, 4), (5, 10), bounds, |candidate| {
                rectangles_overlap(candidate, bounds).then_some(bounds)
            }),
            None
        );
    }

    #[test]
    fn rectangle_placement_uses_discrete_line_obstacles() {
        assert_eq!(
            place_rect_avoiding(
                Rect::new(0, 0, 4, 4),
                (5, 5),
                Rect::new(0, 0, 20, 20),
                |candidate| line_obstacle(candidate, (0, 6), (19, 6)),
            ),
            Some(Rect::new(5, 7, 4, 4))
        );
    }

    #[test]
    fn rectangle_placement_rejects_invalid_extents_and_obstacles() {
        let bounds = Rect::new(0, 0, 20, 20);
        assert_eq!(
            place_rect_avoiding(Rect::new(0, 0, 0, 4), (0, 0), bounds, |_| None),
            None
        );
        assert_eq!(
            place_rect_avoiding(Rect::new(0, 0, 4, 4), (0, 0), bounds, |_| Some(Rect::new(
                10, 10, 2, 2
            )),),
            None
        );
    }
}
