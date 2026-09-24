#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(unsafe_code)]

mod greet;

pub use greet::greet;

#[cfg(target_arch = "wasm32")]
mod browser;
