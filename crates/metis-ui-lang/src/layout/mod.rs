//! Sequential row/column box layout and display-list generation.
//!
//! [`VirtualList`] windows long lists so a host builds only visible items.
//!
//! Supports explicit, automatic and minimum sizes, margins, padding, flex
//! alignment, rounded backgrounds and uniform borders, outer box shadows and
//! text. A visible `popover-anchor` element leaves normal flow and paints after
//! the document at its laid-out anchor. Every declaration the style subset
//! admits is painted; anything outside it is rejected when the style is parsed.

mod damage;
mod device;
mod display;
mod geometry;
mod intrinsic;
mod limits;
mod popover;
#[cfg(test)]
#[path = "../layout_popover_tests.rs"]
mod popover_tests;
#[cfg(test)]
#[path = "../layout_sizing_tests.rs"]
mod sizing_tests;
#[cfg(test)]
#[path = "../layout_tests.rs"]
mod tests;
mod virtual_list;

pub use display::{DisplayCommand, DisplayList};
pub use geometry::{LayoutViewport, compute_layout};
pub use metis_platform::framebuffer::Rect;
pub use metis_platform::rasterizer::{LineCap, LineJoin, StrokeWidth};
pub use virtual_list::{
    MAX_ITEM_EXTENT, MAX_VIRTUAL_ITEMS, ScrollAlign, VirtualList, VisibleWindow,
};
