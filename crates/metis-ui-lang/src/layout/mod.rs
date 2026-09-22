//! Sequential row/column box layout and display-list generation.
//!
//! Supports explicit, automatic and minimum sizes, margins, padding, flex
//! alignment, rounded backgrounds and uniform borders, outer box shadows and
//! text. Every declaration the style subset admits is painted; anything
//! outside it is rejected when the style is parsed.

mod device;
mod display;
mod geometry;
#[cfg(test)]
#[path = "../layout_sizing_tests.rs"]
mod sizing_tests;
#[cfg(test)]
#[path = "../layout_tests.rs"]
mod tests;

pub use display::{DisplayCommand, DisplayList};
pub use geometry::{LayoutViewport, compute_layout};
pub use metis_platform::framebuffer::Rect;
pub use metis_platform::rasterizer::{LineCap, LineJoin, StrokeWidth};
