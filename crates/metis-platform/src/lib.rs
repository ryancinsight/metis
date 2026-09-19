#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod display_scale;
pub mod event;
pub mod font;
pub mod framebuffer;
pub mod rasterizer;
#[cfg(not(target_arch = "wasm32"))]
pub mod scoped_file;
pub mod surface;

#[cfg(windows)]
pub mod native;

pub use display_scale::DisplayScale;
pub use event::PlatformEvent;
pub use font::{FONT_HEIGHT, FONT_WIDTH, draw_glyph};
pub use framebuffer::{Color, Framebuffer, Rect};
pub use rasterizer::{
    LineCap, LineJoin, MAX_STROKE_POINTS, StrokeWidth, draw_line, draw_polyline, draw_rect_outline,
    draw_text, draw_text_scaled, fill_rect,
};
#[cfg(not(target_arch = "wasm32"))]
pub use scoped_file::{MAX_SCOPED_FILE_BYTES, ScopedFileProvider};
pub use surface::PlatformSurface;
