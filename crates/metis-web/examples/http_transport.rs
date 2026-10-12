//! Real-browser Fetch bound, deadline and cancellation oracles.
#![deny(unsafe_code)]

#[cfg(target_arch = "wasm32")]
#[path = "http_transport/browser.rs"]
mod browser;
