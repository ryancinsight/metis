//! HTML5/CSS browser host for Metis WASM applications.
#![deny(missing_docs)]
#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

pub mod theme;

mod canvas;

pub use canvas::CanvasFrame;

#[cfg(target_arch = "wasm32")]
pub use canvas::CanvasSurface;

mod file_batch;

pub use file_batch::{FileDropBatch, FileDropPayload};

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

#[cfg(any(target_arch = "wasm32", test))]
#[path = "browser/fragment.rs"]
mod fragment;

#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(target_arch = "wasm32")]
pub use browser::{metis_start, metis_stop};

/// Takes the latest completed browser file batch, if one is waiting.
///
/// The browser host keeps one bounded handoff slot so a decoder can poll from
/// its own application loop without registering an unbounded callback. Taking
/// the value transfers ownership of the bytes to the caller. A later drop
/// replaces an unconsumed batch, and [`metis_stop`] drops any remaining batch.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn take_file_drop() -> Option<FileDropBatch> {
    browser::take_file_drop()
}

pub use theme::Theme;
