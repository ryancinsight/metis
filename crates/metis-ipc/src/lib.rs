#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod client;
pub mod fault;
pub mod frame;
pub mod server;
pub mod transport;

pub use client::IpcClient;
pub use fault::{FaultConfig, FaultInjectingTransport};
pub use frame::{read_frame, write_frame};
pub use server::{IpcHandler, IpcServer};
pub use transport::{IpcTransport, MemoryTransport, StreamTransport};
