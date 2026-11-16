//! Geometry and pointer mapping for bounded scrollbars.

use metis_platform::framebuffer::Rect;

/// Axis on which a scrollbar moves its thumb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarAxis {
    /// A top-to-bottom scrollbar.
    Vertical,
    /// A left-to-right scrollbar.
    Horizontal,
}

/// A painted scrollbar's track and thumb, with drag mapping for its owner.
///
/// The scrollbar does not retain the content offset or pointer-capture state.
/// A host keeps those values and passes the pointer position and grab offset to
/// [`Self::offset_for_pointer`] while a drag is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollbarGeometry {
    /// The axis along which the thumb moves.
    axis: ScrollbarAxis,
    /// The full painted track.
    track: Rect,
    /// The current thumb position.
    thumb: Rect,
    /// Largest content offset represented by this scrollbar.
    max_offset: u64,
}

impl ScrollbarGeometry {
    /// Minimum thumb extent in logical pixels.
    pub const MIN_THUMB_EXTENT: i32 = 16;

    /// Computes geometry for a content extent and viewport.
    ///
    /// Returns `None` when scrolling is unnecessary, the track is empty, or
    /// the dimensions cannot be represented by the track rectangle.
    #[must_use]
    pub fn new(
        axis: ScrollbarAxis,
        track: Rect,
        content_extent: u64,
        viewport_extent: u32,
        offset: u64,
    ) -> Option<Self> {
        let (track_start, track_extent) = match axis {
            ScrollbarAxis::Vertical => (i64::from(track.y), track.height),
            ScrollbarAxis::Horizontal => (i64::from(track.x), track.width),
        };
        let cross_extent = match axis {
            ScrollbarAxis::Vertical => track.width,
            ScrollbarAxis::Horizontal => track.height,
        };
        if track_extent <= 0
            || cross_extent <= 0
            || viewport_extent == 0
            || content_extent <= u64::from(viewport_extent)
        {
            return None;
        }
        let track_extent_u64 = u64::try_from(track_extent).ok()?;
        let natural_thumb = u64::try_from(
            u128::from(track_extent_u64) * u128::from(viewport_extent) / u128::from(content_extent),
        )
        .ok()?;
        let min_thumb = u64::try_from(Self::MIN_THUMB_EXTENT).ok()?;
        let thumb_extent = natural_thumb.max(min_thumb).min(track_extent_u64);
        if thumb_extent >= track_extent_u64 {
            return None;
        }
        let travel = track_extent_u64.saturating_sub(thumb_extent);
        let max_offset = content_extent.saturating_sub(u64::from(viewport_extent));
        let clamped_offset = offset.min(max_offset);
        let thumb_start = track_start.checked_add(
            i64::try_from(
                u64::try_from(
                    u128::from(travel) * u128::from(clamped_offset) / u128::from(max_offset),
                )
                .ok()?,
            )
            .ok()?,
        )?;
        let thumb_start = i32::try_from(thumb_start).ok()?;
        let thumb_extent = i32::try_from(thumb_extent).ok()?;
        let thumb = match axis {
            ScrollbarAxis::Vertical => Rect::new(track.x, thumb_start, track.width, thumb_extent),
            ScrollbarAxis::Horizontal => {
                Rect::new(thumb_start, track.y, thumb_extent, track.height)
            }
        };
        Some(Self {
            axis,
            track,
            thumb,
            max_offset,
        })
    }

    /// Returns the scrollbar axis.
    #[must_use]
    pub const fn axis(self) -> ScrollbarAxis {
        self.axis
    }

    /// Returns the full painted track.
    #[must_use]
    pub const fn track(self) -> Rect {
        self.track
    }

    /// Returns the current thumb rectangle.
    #[must_use]
    pub const fn thumb(self) -> Rect {
        self.thumb
    }

    /// Returns the largest representable content offset.
    #[must_use]
    pub const fn max_offset(self) -> u64 {
        self.max_offset
    }

    /// Returns whether a pointer lies in the thumb.
    #[must_use]
    pub fn thumb_contains(self, x: i32, y: i32) -> bool {
        self.thumb.contains(x, y)
    }

    /// Returns whether a pointer lies in the track.
    #[must_use]
    pub fn track_contains(self, x: i32, y: i32) -> bool {
        self.track.contains(x, y)
    }

    /// Returns the pointer's grab distance from the thumb's leading edge.
    ///
    /// The returned value is suitable for passing as `grab_offset` to
    /// [`Self::offset_for_pointer`] after pointer capture begins.
    #[must_use]
    pub fn grab_offset(self, x: i32, y: i32) -> Option<i32> {
        if !self.thumb_contains(x, y) {
            return None;
        }
        let offset = match self.axis {
            ScrollbarAxis::Vertical => y.saturating_sub(self.thumb.y),
            ScrollbarAxis::Horizontal => x.saturating_sub(self.thumb.x),
        };
        Some(offset)
    }

    /// Maps a pointer coordinate to a content offset during a drag.
    ///
    /// `grab_offset` is the pointer's distance from the thumb's leading edge
    /// recorded when the host captured the thumb. The result is clamped to the
    /// track and content range.
    ///
    /// # Panics
    /// Panics only if an internal nonnegative conversion or clamped offset
    /// exceeds its declared integer range.
    #[must_use]
    pub fn offset_for_pointer(self, pointer: i32, grab_offset: i32) -> u64 {
        let (track_start, track_extent, thumb_extent) = match self.axis {
            ScrollbarAxis::Vertical => (self.track.y, self.track.height, self.thumb.height),
            ScrollbarAxis::Horizontal => (self.track.x, self.track.width, self.thumb.width),
        };
        let travel = i64::from(track_extent.saturating_sub(thumb_extent));
        if travel <= 0 || self.max_offset == 0 {
            return 0;
        }
        let thumb_start = i64::from(pointer)
            .saturating_sub(i64::from(grab_offset))
            .saturating_sub(i64::from(track_start));
        let position = thumb_start.clamp(0, travel);
        let position =
            u128::try_from(position).expect("invariant: clamped scrollbar position is nonnegative");
        let travel = u128::try_from(travel).expect("invariant: scrollbar travel is positive");
        let offset = position * u128::from(self.max_offset) / travel;
        u64::try_from(offset).expect("invariant: scrollbar ratio cannot exceed max offset")
    }

    /// Captures pointer state for drag updates with stationary-value stability.
    #[must_use]
    pub fn begin_drag(self, pointer: i32, grab_offset: i32, offset: u64) -> ScrollbarDrag {
        ScrollbarDrag {
            geometry: self,
            pointer,
            grab_offset,
            offset: offset.min(self.max_offset),
        }
    }
}

/// Pointer-captured state for a scrollbar drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollbarDrag {
    geometry: ScrollbarGeometry,
    pointer: i32,
    grab_offset: i32,
    offset: u64,
}

impl ScrollbarDrag {
    /// Maps a new pointer coordinate while preserving the captured offset.
    ///
    /// # Panics
    /// Panics only if the clamped offset exceeds `u64`, which cannot occur for
    /// the geometry's `u64` maximum offset.
    #[must_use]
    pub fn offset_for_pointer(self, pointer: i32) -> u64 {
        let (track_start, track_extent, thumb_extent) = match self.geometry.axis {
            ScrollbarAxis::Vertical => (
                self.geometry.track.y,
                self.geometry.track.height,
                self.geometry.thumb.height,
            ),
            ScrollbarAxis::Horizontal => (
                self.geometry.track.x,
                self.geometry.track.width,
                self.geometry.thumb.width,
            ),
        };
        let travel = i128::from(track_extent.saturating_sub(thumb_extent));
        if travel <= 0 {
            return self.offset;
        }
        let leading_edge = i128::from(pointer).saturating_sub(i128::from(self.grab_offset));
        let track_start = i128::from(track_start);
        if leading_edge <= track_start {
            return 0;
        }
        if leading_edge >= track_start.saturating_add(travel) {
            return self.geometry.max_offset;
        }
        let delta = i128::from(pointer).saturating_sub(i128::from(self.pointer));
        let change = delta * i128::from(self.geometry.max_offset) / travel;
        let result = i128::from(self.offset).saturating_add(change);
        u64::try_from(result.clamp(0, i128::from(self.geometry.max_offset)))
            .expect("invariant: clamped scrollbar offset is representable")
    }

    /// Returns the captured pointer grab distance.
    #[must_use]
    pub const fn grab_offset(self) -> i32 {
        self.grab_offset
    }
}

#[cfg(test)]
mod tests {
    use super::{ScrollbarAxis, ScrollbarGeometry};
    use metis_platform::framebuffer::Rect;

    #[test]
    fn geometry_clamps_thumb_and_offset() {
        let geometry = ScrollbarGeometry::new(
            ScrollbarAxis::Vertical,
            Rect::new(10, 20, 12, 100),
            1_000,
            100,
            900,
        )
        .expect("invariant: content exceeds viewport and track is positive");
        assert_eq!(geometry.thumb(), Rect::new(10, 104, 12, 16));
        assert_eq!(geometry.max_offset(), 900);
        assert_eq!(geometry.offset_for_pointer(120, 8), 900);
        assert_eq!(geometry.offset_for_pointer(204, 8), 900);
        let intermediate = ScrollbarGeometry::new(
            ScrollbarAxis::Vertical,
            Rect::new(10, 20, 12, 100),
            1_000,
            100,
            451,
        )
        .expect("invariant: content exceeds viewport and track is positive");
        assert_eq!(intermediate.thumb().y, 62);
        let drag = intermediate.begin_drag(70, 8, 451);
        assert_eq!(drag.offset_for_pointer(70), 451);
        assert_eq!(drag.offset_for_pointer(28), 0);
        assert_eq!(drag.offset_for_pointer(154), 900);
    }

    #[test]
    fn short_content_has_no_scrollbar() {
        assert!(
            ScrollbarGeometry::new(
                ScrollbarAxis::Horizontal,
                Rect::new(0, 0, 100, 12),
                100,
                100,
                0,
            )
            .is_none()
        );
    }

    #[test]
    fn horizontal_geometry_supports_wide_content() {
        let geometry = ScrollbarGeometry::new(
            ScrollbarAxis::Horizontal,
            Rect::new(-20, 4, 200, 8),
            u64::MAX,
            1,
            u64::MAX - 1,
        )
        .expect("invariant: positive cross-axis and scrollable content");
        assert!(geometry.thumb().x < 180);
        assert_eq!(geometry.offset_for_pointer(180, 16), geometry.max_offset());
    }

    #[test]
    fn track_without_cross_axis_extent_has_no_scrollbar() {
        assert!(
            ScrollbarGeometry::new(
                ScrollbarAxis::Vertical,
                Rect::new(0, 0, 0, 100),
                200,
                100,
                0,
            )
            .is_none()
        );
    }
}
