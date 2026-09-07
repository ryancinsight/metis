#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod async_client;
pub mod client;
pub mod fault;
pub mod frame;
pub mod server;
pub mod transport;

#[cfg(target_arch = "wasm32")]
mod browser;

pub use async_client::AsyncIpcClient;
pub use client::IpcClient;
pub use fault::{FaultConfig, FaultInjectingTransport};
pub use frame::{read_frame, write_frame};
pub use server::{IpcHandler, IpcServer};
pub use transport::{AsyncIpcTransport, IpcTransport, MemoryTransport, StreamTransport};

#[cfg(target_arch = "wasm32")]
pub use browser::BrowserWebSocketTransport;
