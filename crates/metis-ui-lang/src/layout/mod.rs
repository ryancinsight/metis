//! Sequential row/column box layout and display-list generation.
//!
//! [`VirtualList`] windows long lists so a host builds only visible items.
//!
//! Supports explicit, automatic and minimum sizes, margins, padding, flex
//! growth and alignment, rounded backgrounds and uniform borders, outer box shadows and
//! text. A visible `popover-anchor` element leaves normal flow and paints after
//! the document at its laid-out anchor. Every declaration the style subset
//! admits is painted; anything outside it is rejected when the style is parsed.

mod damage;
mod decimal_entry;
mod device;
mod display;
#[cfg(test)]
#[path = "../layout_display_tests.rs"]
mod display_tests;
mod geometry;
mod grow;
#[cfg(test)]
#[path = "../layout_grow_tests.rs"]
mod grow_tests;
mod intrinsic;
mod limits;
mod path;
mod placement;
mod popover;
#[cfg(test)]
#[path = "../layout_popover_tests.rs"]
mod popover_tests;
mod scrollbar;
#[cfg(test)]
#[path = "../layout_sizing_tests.rs"]
mod sizing_tests;
mod slots;
#[cfg(test)]
#[path = "../layout_tests.rs"]
mod tests;
mod virtual_list;

pub use decimal_entry::{DecimalEdit, DecimalEntry};
pub use display::{DisplayCommand, DisplayList, ElementIndex};
pub use geometry::{LayoutViewport, compute_layout, compute_layout_into};
pub use metis_platform::framebuffer::Rect;
pub use metis_platform::rasterizer::{LineCap, LineJoin, StrokeWidth};
pub use path::{PathCapture, PathPush, PathTopology, PolylineError, polyline_distance};
pub use scrollbar::{ScrollbarAxis, ScrollbarDrag, ScrollbarGeometry};
pub use virtual_list::{
    MAX_ITEM_EXTENT, MAX_VIRTUAL_ITEMS, ScrollAlign, VirtualList, VisibleWindow,
};
