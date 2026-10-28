//! Per-container child-placement records and the one arena that stores them.
//!
//! The records are the layout pass's scratch: where each child's commands
//! landed and what the container has to redistribute. One arena serves the
//! whole tree, sized once from the node count.

use std::ops::Range;

use crate::parser::limit_error;
use crate::style::ComputedStyle;
use metis_core::error::Result;
use metis_platform::DisplayScale;

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
