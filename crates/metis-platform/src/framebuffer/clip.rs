//! The region of a framebuffer that writes may change.

use super::Framebuffer;

/// The half-open pixel region writes may change.
///
/// Rasterizers bound their visible rows and columns by it, so narrowing the
/// clip narrows their work as well as their writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clip {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

impl Clip {
    pub(super) const fn new(left: u32, top: u32, right: u32, bottom: u32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
    /// First writable column.
    #[must_use]
    pub const fn left(self) -> u32 {
        self.left
    }
    /// First writable row.
    #[must_use]
    pub const fn top(self) -> u32 {
        self.top
    }
    /// Column after the last writable one.
    #[must_use]
    pub const fn right(self) -> u32 {
        self.right
    }
    /// Row after the last writable one.
    #[must_use]
    pub const fn bottom(self) -> u32 {
        self.bottom
    }
    /// Reports whether no pixel is writable.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }
    pub(super) const fn contains(self, x: u32, y: u32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

/// Restores the whole-surface clip when a clipped render ends, unwinding
/// included, so a panic inside it cannot leave later writes silently narrowed.
pub(super) struct ClipScope<'surface> {
    pub(super) surface: &'surface mut Framebuffer,
}

impl<'surface> ClipScope<'surface> {
    pub(super) const fn narrow(surface: &'surface mut Framebuffer, clip: Clip) -> Self {
        surface.set_clip(clip);
        Self { surface }
    }
}

impl Drop for ClipScope<'_> {
    fn drop(&mut self) {
        let whole = self.surface.surface();
        self.surface.set_clip(whole);
    }
}
