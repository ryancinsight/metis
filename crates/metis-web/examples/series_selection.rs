//! Opt-in series/viewport selection demonstration; no DICOM processing.
#![deny(unsafe_code)]

#[cfg(any(target_arch = "wasm32", test))]
#[path = "series_selection/model.rs"]
mod model;

#[cfg(target_arch = "wasm32")]
#[path = "series_selection/presentation.rs"]
mod presentation;

#[cfg(target_arch = "wasm32")]
#[path = "series_selection/browser.rs"]
mod browser;

#[cfg(test)]
#[path = "series_selection/tests.rs"]
mod tests;
