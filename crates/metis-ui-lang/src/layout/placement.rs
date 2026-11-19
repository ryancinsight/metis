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

/// Horizontal direction used while searching for a free rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalSearch {
    /// Search toward increasing x coordinates.
    Increasing,
    /// Search toward decreasing x coordinates.
    Decreasing,
}

/// Places a measured rectangle inside bounds without crossing caller-owned obstacles.
///
/// `extent` is relative to `origin`, including any left or top bearing. The
/// callback returns `None` when a candidate is clear or the next candidate x
/// beyond the blocking obstacle. Increasing searches require a greater x and
/// decreasing searches require a smaller x; a non-advancing response rejects
/// the placement. Rows start at the requested origin, continue downward, then
/// search upward. Each row searches right before falling back to the left.
#[must_use]
pub fn place_rect_avoiding(
    extent: Rect,
    origin: (i32, i32),
    bounds: Rect,
    mut next_x: impl FnMut(Rect, HorizontalSearch) -> Option<i32>,
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
    let right = bounds
        .x
        .checked_add(bounds.width)?
        .checked_sub(extent.width)?;
    let bottom = bounds
        .y
        .checked_add(bounds.height)?
        .checked_sub(extent.height)?;
    let x = origin.0.saturating_add(extent.x).clamp(bounds.x, right);
    let y = origin.1.saturating_add(extent.y).clamp(bounds.y, bottom);
    for row in (y..=bottom).chain((bounds.y..y).rev()) {
        let mut candidate = Rect::new(x, row, extent.width, extent.height);
        while candidate.x <= right {
            let Some(next) = next_x(candidate, HorizontalSearch::Increasing) else {
                return Some(candidate);
            };
            if next <= candidate.x {
                return None;
            }
            candidate.x = next;
        }
        candidate.x = x;
        while candidate.x >= bounds.x {
            let Some(next) = next_x(candidate, HorizontalSearch::Decreasing) else {
                return Some(candidate);
            };
            if next >= candidate.x {
                return None;
            }
            candidate.x = next;
        }
    }
    None
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

    fn overlaps(left: Rect, right: Rect) -> bool {
        left.x < right.x + right.width
            && right.x < left.x + left.width
            && left.y < right.y + right.height
            && right.y < left.y + left.height
    }

    fn next_text_x(
        candidate: Rect,
        direction: HorizontalSearch,
        obstacles: &[Rect],
    ) -> Option<i32> {
        let overlapping = obstacles
            .iter()
            .copied()
            .filter(|obstacle| overlaps(candidate, *obstacle));
        match direction {
            HorizontalSearch::Increasing => overlapping
                .map(|obstacle| obstacle.x + obstacle.width)
                .max(),
            HorizontalSearch::Decreasing => overlapping
                .map(|obstacle| obstacle.x - candidate.width)
                .min(),
        }
    }

    #[test]
    fn rectangle_placement_preserves_text_collision_search_order() {
        let bounds = Rect::new(0, 0, 80, 40);
        let extent = Rect::new(-1, -2, 12, 8);
        let right_obstacles = [Rect::new(20, 10, 20, 8), Rect::new(40, 10, 20, 8)];
        assert_eq!(
            place_rect_avoiding(extent, (22, 12), bounds, |candidate, direction| {
                next_text_x(candidate, direction, &right_obstacles)
            }),
            Some(Rect::new(60, 10, 12, 8))
        );

        let spanning = [Rect::new(21, 10, 59, 8)];
        assert_eq!(
            place_rect_avoiding(extent, (22, 12), bounds, |candidate, direction| {
                next_text_x(candidate, direction, &spanning)
            }),
            Some(Rect::new(9, 10, 12, 8))
        );

        let full_row = [Rect::new(0, 10, 80, 8)];
        assert_eq!(
            place_rect_avoiding(extent, (22, 12), bounds, |candidate, direction| {
                next_text_x(candidate, direction, &full_row)
            }),
            Some(Rect::new(21, 18, 12, 8))
        );
    }

    #[test]
    fn rectangle_placement_rejects_invalid_extents_and_nonadvancing_obstacles() {
        let bounds = Rect::new(0, 0, 20, 20);
        assert_eq!(
            place_rect_avoiding(Rect::new(0, 0, 0, 4), (0, 0), bounds, |_, _| None),
            None
        );
        assert_eq!(
            place_rect_avoiding(Rect::new(0, 0, 4, 4), (0, 0), bounds, |candidate, _| {
                Some(candidate.x)
            }),
            None
        );
        assert_eq!(
            place_rect_avoiding(
                Rect::new(0, 0, 4, 4),
                (i32::MAX, i32::MAX),
                Rect::new(i32::MAX - 1, i32::MAX - 1, 4, 4),
                |_, _| None,
            ),
            None
        );
    }
}
