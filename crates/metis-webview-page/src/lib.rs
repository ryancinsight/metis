//! The packaged `WebView2` page presentation, compiled to WebAssembly.
//!
//! METIS-WEBVIEW-RUST-001 replaces the previously embedded JavaScript page
//! scripts with this Rust module: the same form lifecycle (theme, command
//! menu, announced keyboard shortcuts, calculation submit and typed reply
//! rendering) and the same bounded permission-probe lifecycle run from
//! WebAssembly through typed browser bindings. The generated `wasm-bindgen`
//! loader (`scripts/webview_page.py`) remains generated interop; the Rust
//! presentation below owns every behavioral line. The page reports malformed
//! host replies and releases the submit button so the user can retry.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

#[cfg(any(test, target_arch = "wasm32"))]
mod message;

#[cfg(target_arch = "wasm32")]
mod page;

/// Pure page-domain contracts: the crate's public native API surface, with
/// unit tests that run on every target.
pub mod logic;

/// Runs the packaged page when the generated loader instantiates the module.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    page::start();
}
