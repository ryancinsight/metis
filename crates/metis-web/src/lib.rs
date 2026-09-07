//! HTML5/CSS browser host for Metis WASM applications.
#![deny(missing_docs)]
#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(target_arch = "wasm32")]
pub use browser::metis_start;
