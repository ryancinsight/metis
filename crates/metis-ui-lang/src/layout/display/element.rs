//! Element attribution for a display list.

use crate::transition::Transition;
use std::sync::Arc;

/// Position of an element in a display list's element table.
///
/// Layout numbers each element as it emits it, so the number is stable for the
/// document revision and viewport the list was laid out at, which is exactly
/// the lifetime a paint-time transition overlay spans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElementIndex(pub(super) u32);

impl ElementIndex {
    /// The element's position in emission order.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// An element whose style declares a transition.
#[derive(Debug, Clone)]
pub(super) struct AnimatedElement {
    /// Authored identifier; `None` for an element the document did not name,
    /// which no run can follow across a relayout.
    pub(super) id: Option<Arc<str>>,
    /// The transition the element's style declares.
    pub(super) transition: Transition,
}
