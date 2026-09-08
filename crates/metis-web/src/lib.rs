//! HTML5/CSS browser host for Metis WASM applications.
#![deny(missing_docs)]
#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

pub mod theme;

#[cfg(any(target_arch = "wasm32", test))]
mod epoch;

#[cfg(any(target_arch = "wasm32", test))]
mod session;

#[cfg(any(target_arch = "wasm32", test))]
mod controls;

#[cfg(any(target_arch = "wasm32", test))]
#[path = "browser/gesture_policy.rs"]
mod gesture_policy;

#[cfg(any(target_arch = "wasm32", test))]
#[path = "browser/file_drop_policy.rs"]
mod file_drop_policy;

#[cfg(any(target_arch = "wasm32", test))]
#[path = "browser/text_policy.rs"]
mod text_policy;

#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(target_arch = "wasm32")]
pub use browser::{metis_start, metis_stop};

pub use theme::Theme;
