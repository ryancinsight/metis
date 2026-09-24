//! The part of a framebuffer a change has made stale.

use super::Rect;

/// What a repaint or a presentation must cover to bring a surface up to date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Damage {
    /// No pixel changed.
    Unchanged,
    /// Only pixels inside this device rectangle can have changed.
    Region(Rect),
    /// Any pixel can have changed.
    Full,
}

impl Damage {
    /// Damage covering everything either covers.
    ///
    /// Two changes made before one presentation merge, so the presentation
    /// covers both.
    ///
    /// # Examples
    ///
    /// ```
    /// use metis_platform::framebuffer::{Damage, Rect};
    ///
    /// let first = Damage::Region(Rect::new(0, 0, 4, 4));
    /// let second = Damage::Region(Rect::new(10, 2, 4, 4));
    /// assert_eq!(first.merge(second), Damage::Region(Rect::new(0, 0, 14, 6)));
    /// assert_eq!(first.merge(Damage::Unchanged), first);
    /// assert_eq!(first.merge(Damage::Full), Damage::Full);
    /// ```
    #[must_use]
    pub fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Full, _) | (_, Self::Full) => Self::Full,
            (Self::Unchanged, damage) | (damage, Self::Unchanged) => damage,
            (Self::Region(first), Self::Region(second)) => Self::Region(first.union(second)),
        }
    }
}

impl Rect {
    /// The smallest rectangle holding both, saturated to the `i32` range.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        let (left, top) = (self.x.min(other.x), self.y.min(other.y));
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        // Saturation is the contract: an extent past `i32::MAX` is `i32::MAX`.
        let extent = |start: i32, end: i64| {
            i32::try_from((end - i64::from(start)).max(0)).unwrap_or(i32::MAX)
        };
        Self::new(left, top, extent(left, right), extent(top, bottom))
    }

    /// Reports whether every pixel of `other` lies in `self`.
    #[must_use]
    pub fn covers(self, other: Self) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && self.right() >= other.right()
            && self.bottom() >= other.bottom()
    }

    fn right(self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }

    fn bottom(self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }
}
