//! Virtualized list windowing.
//!
//! A long list is laid out as if every item were present, but only the items
//! that intersect the viewport — plus a small overscan margin — are built and
//! painted. This is the role of egui's `ScrollArea::show_rows`, GPUI's
//! `uniform_list`/`list` and the DOM spacer technique: the host renders
//! [`VisibleWindow::items`], offsets them by [`VisibleWindow::start`] and
//! reserves [`VirtualList::total_extent`] so the scrollbar reflects the whole
//! list.
//!
//! Uniform lists answer every query in constant time. Variable lists keep
//! their extents in a Fenwick tree, so locating an offset, measuring a span
//! and updating one measured extent each cost `O(log n)`; no query walks the
//! whole list. Extents are logical pixels along the scroll axis.

mod extent_tree;

use crate::parser::limit_error;
use extent_tree::ExtentTree;
use metis_core::error::Result;
use std::ops::Range;

/// Upper bound on items in one virtual list.
pub const MAX_VIRTUAL_ITEMS: usize = 1 << 20;

/// Upper bound on one item's extent, which keeps every total within `u64`
/// and every item within a single framebuffer's reach.
pub const MAX_ITEM_EXTENT: u32 = 1 << 16;

/// How [`VirtualList::scroll_to`] aligns an item within the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAlign {
    /// Put the item's start at the viewport's start.
    Start,
    /// Center the item in the viewport.
    Center,
    /// Put the item's end at the viewport's end.
    End,
    /// Scroll the least distance that shows the item, or not at all when it
    /// is already fully visible.
    Nearest,
}

/// The items a host must build for one scroll position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleWindow {
    /// Item indices to build, overscan included.
    pub items: Range<usize>,
    /// Content offset of the first built item's start.
    pub start: u64,
    /// Content offset of the last built item's end.
    pub end: u64,
    /// The scroll offset after clamping to the scrollable range.
    pub offset: u64,
}

#[derive(Debug, Clone)]
enum Extents {
    Uniform { count: usize, extent: u32 },
    Variable(ExtentTree),
}

/// Scroll-axis geometry for a list whose items are built on demand.
#[derive(Debug, Clone)]
pub struct VirtualList {
    extents: Extents,
    overscan: usize,
}

impl VirtualList {
    /// A list of `count` items that all share `extent`.
    ///
    /// # Errors
    /// Returns a layout-overflow error when `count` exceeds
    /// [`MAX_VIRTUAL_ITEMS`] or `extent` is zero or above [`MAX_ITEM_EXTENT`].
    pub fn uniform(count: usize, extent: u32) -> Result<Self> {
        validate_count(count)?;
        validate_extent(extent)?;
        Ok(Self {
            extents: Extents::Uniform { count, extent },
            overscan: 0,
        })
    }

    /// A list whose items have individual extents; zero-extent items are
    /// admitted and never intersect the viewport.
    ///
    /// # Errors
    /// Returns a layout-overflow error when there are more than
    /// [`MAX_VIRTUAL_ITEMS`] items, an extent exceeds [`MAX_ITEM_EXTENT`], or
    /// storage cannot be reserved.
    pub fn variable(extents: &[u32]) -> Result<Self> {
        validate_count(extents.len())?;
        for extent in extents {
            validate_extent_or_zero(*extent)?;
        }
        Ok(Self {
            extents: Extents::Variable(ExtentTree::new(extents)?),
            overscan: 0,
        })
    }

    /// Builds `items` extra items on each side of the viewport, so a small
    /// scroll shows content before the host has built the next window.
    #[must_use]
    pub const fn with_overscan(mut self, items: usize) -> Self {
        self.overscan = items;
        self
    }

    /// Number of items.
    #[must_use]
    pub fn len(&self) -> usize {
        match &self.extents {
            Extents::Uniform { count, .. } => *count,
            Extents::Variable(tree) => tree.len(),
        }
    }

    /// Whether the list has no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Sum of every item's extent: the height a host reserves for scrolling.
    #[must_use]
    pub fn total_extent(&self) -> u64 {
        self.prefix(self.len())
    }

    /// The content span of item `index`.
    #[must_use]
    pub fn item_span(&self, index: usize) -> Option<Range<u64>> {
        (index < self.len()).then(|| self.prefix(index)..self.prefix(index + 1))
    }

    /// Replaces one item's extent after the host measures it.
    ///
    /// # Errors
    /// Returns a layout-overflow error for an index outside the list, an
    /// extent above [`MAX_ITEM_EXTENT`], or a uniform list whose shared extent
    /// would change for a single item.
    pub fn set_extent(&mut self, index: usize, extent: u32) -> Result<()> {
        validate_extent_or_zero(extent)?;
        match &mut self.extents {
            Extents::Variable(tree) if index < tree.len() => {
                tree.set(index, extent);
                Ok(())
            }
            Extents::Uniform {
                count,
                extent: shared,
            } if index < *count && *shared == extent => Ok(()),
            Extents::Uniform { .. } => {
                Err(limit_error("Uniform virtual list items share one extent"))
            }
            Extents::Variable(_) => Err(limit_error("Virtual list index is out of range")),
        }
    }

    /// Largest valid scroll offset for a viewport of `viewport` pixels.
    #[must_use]
    pub fn max_offset(&self, viewport: u32) -> u64 {
        self.total_extent().saturating_sub(u64::from(viewport))
    }

    /// The items intersecting `[offset, offset + viewport)` after clamping
    /// `offset`, widened by the overscan.
    #[must_use]
    pub fn window(&self, offset: u64, viewport: u32) -> VisibleWindow {
        let offset = offset.min(self.max_offset(viewport));
        let len = self.len();
        let first = self.index_at(offset);
        if first >= len || viewport == 0 {
            // Nothing intersects: an empty list, a zero viewport, or only
            // zero-extent items at and after the offset.
            let start = self.prefix(first);
            return VisibleWindow {
                items: first.min(len)..first.min(len),
                start,
                end: start,
                offset,
            };
        }
        let last_offset = offset + u64::from(viewport) - 1;
        let last = self.index_at(last_offset).min(len - 1);
        let items = first.saturating_sub(self.overscan)..(last + 1 + self.overscan).min(len);
        VisibleWindow {
            start: self.prefix(items.start),
            end: self.prefix(items.end),
            items,
            offset,
        }
    }

    /// The scroll offset that shows item `index` under `align`, given the
    /// `current` offset; `None` for an index outside the list.
    #[must_use]
    pub fn scroll_to(
        &self,
        index: usize,
        viewport: u32,
        current: u64,
        align: ScrollAlign,
    ) -> Option<u64> {
        let span = self.item_span(index)?;
        let viewport_extent = u64::from(viewport);
        let target = match align {
            ScrollAlign::Start => span.start,
            ScrollAlign::End => span.end.saturating_sub(viewport_extent),
            ScrollAlign::Center => {
                let middle = span.start + (span.end - span.start) / 2;
                middle.saturating_sub(viewport_extent / 2)
            }
            ScrollAlign::Nearest => {
                let current = current.min(self.max_offset(viewport));
                if span.start < current {
                    span.start
                } else if span.end > current + viewport_extent {
                    // An item taller than the viewport shows its start.
                    span.end.saturating_sub(viewport_extent).min(span.start)
                } else {
                    current
                }
            }
        };
        Some(target.min(self.max_offset(viewport)))
    }

    /// Content offset where item `index` starts; `len` gives the total.
    fn prefix(&self, index: usize) -> u64 {
        match &self.extents {
            Extents::Uniform { count, extent } => {
                u64::try_from(index.min(*count)).unwrap_or(u64::MAX) * u64::from(*extent)
            }
            Extents::Variable(tree) => tree.prefix(index),
        }
    }

    /// Index of the item containing `offset`, or `len` past the end.
    fn index_at(&self, offset: u64) -> usize {
        match &self.extents {
            Extents::Uniform { count, extent } => usize::try_from(offset / u64::from(*extent))
                .map_or(*count, |index| index.min(*count)),
            Extents::Variable(tree) => tree.index_at(offset),
        }
    }
}

fn validate_count(count: usize) -> Result<()> {
    if count > MAX_VIRTUAL_ITEMS {
        return Err(limit_error("Virtual list item count exceeds its bound"));
    }
    Ok(())
}

fn validate_extent(extent: u32) -> Result<()> {
    if extent == 0 {
        return Err(limit_error("Uniform virtual list extent must be positive"));
    }
    validate_extent_or_zero(extent)
}

fn validate_extent_or_zero(extent: u32) -> Result<()> {
    if extent > MAX_ITEM_EXTENT {
        return Err(limit_error("Virtual list item extent exceeds its bound"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
