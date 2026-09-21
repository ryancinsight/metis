//! Sequential row/column box layout and display-list generation.
//!
//! Supports explicit or automatic sizes, margins, padding, backgrounds, text, and
//! uniform square borders. Unsupported browser layout declarations are rejected
//! before a display list is emitted so programmatic DOMs cannot silently diverge.

mod display;
mod geometry;
#[cfg(test)]
#[path = "../layout_tests.rs"]
mod tests;

pub use display::{DisplayCommand, DisplayList};
pub use geometry::{LayoutViewport, compute_layout};
pub use metis_platform::framebuffer::Rect;
pub use metis_platform::rasterizer::{LineCap, LineJoin, StrokeWidth};
