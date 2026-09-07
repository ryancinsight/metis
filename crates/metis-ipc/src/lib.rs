#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod async_client;
#[cfg(not(target_arch = "wasm32"))]
pub mod async_server;
pub mod client;
pub mod events;
pub mod fault;
pub mod frame;
pub mod server;
pub mod transport;

#[cfg(target_arch = "wasm32")]
mod browser;

pub use async_client::{AsyncIpcClient, MAX_PENDING_REQUESTS, MAX_QUEUED_EVENTS, RequestId};
#[cfg(not(target_arch = "wasm32"))]
pub use async_server::AsyncIpcServer;
pub use client::{CapabilityError, HandshakeError, IpcClient};
pub use events::{EventHub, MAX_SUBSCRIPTIONS, Subscription, SubscriptionId};
pub use fault::{FaultConfig, FaultInjectingTransport};
pub use frame::{read_frame, write_frame};
pub use server::{IpcHandler, IpcServer};
pub use transport::{AsyncIpcTransport, IpcTransport, MemoryTransport, StreamTransport};

#[cfg(target_arch = "wasm32")]
pub use browser::BrowserWebSocketTransport;
