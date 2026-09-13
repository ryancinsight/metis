//! Thin Python access to the Rust-owned native application host.

#[cfg(windows)]
mod events;
#[cfg(windows)]
mod host;
#[cfg(not(windows))]
mod unsupported;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub(crate) use unsupported::{NativeApplication, assert_thread_safe};
#[cfg(windows)]
pub(crate) use windows::{NativeApplication, assert_thread_safe};
