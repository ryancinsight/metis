#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod event;
pub mod font;
pub mod framebuffer;
pub mod rasterizer;
pub mod surface;

#[cfg(windows)]
pub mod native;

pub use event::PlatformEvent;
pub use font::{FONT_HEIGHT, FONT_WIDTH, draw_glyph};
pub use framebuffer::{Color, Framebuffer, Rect};
pub use rasterizer::{draw_rect_outline, draw_text, fill_rect};
pub use surface::PlatformSurface;
