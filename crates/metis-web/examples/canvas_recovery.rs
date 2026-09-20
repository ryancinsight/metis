//! Browser device-loss demonstration driven by `scripts/browser_gpu_recovery.py`.
#![deny(unsafe_code)]

#[cfg(target_arch = "wasm32")]
#[path = "canvas_recovery/browser.rs"]
mod browser;
