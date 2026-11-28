//! GPU-free browser canvas input fixture driven by `scripts/browser_canvas_input.py`.
#![deny(unsafe_code)]

#[cfg(target_arch = "wasm32")]
#[path = "../src/canvas/mod.rs"]
pub mod canvas;

#[cfg(target_arch = "wasm32")]
#[path = "canvas_input/browser.rs"]
mod browser;
