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
#[cfg(all(feature = "quic", not(target_arch = "wasm32")))]
pub mod quic;
pub mod server;
#[cfg(not(target_arch = "wasm32"))]
pub mod session_transport;
pub mod transport;

#[cfg(target_arch = "wasm32")]
mod browser;

pub use async_client::{AsyncIpcClient, MAX_PENDING_REQUESTS, RequestId};
#[cfg(not(target_arch = "wasm32"))]
pub use async_server::{AsyncIpcServer, MAX_CLINICAL_RESPONSE_DELAY};
pub use client::{
    CapabilityError, HandshakeError, IpcClient, MAX_QUEUED_EVENTS, PluginInvocationError,
    TargetCapabilityError,
};
pub use events::{EventHub, MAX_SUBSCRIPTIONS, Subscription, SubscriptionId};
pub use fault::{FaultConfig, FaultInjectingTransport};
pub use frame::{read_frame, write_frame};
#[cfg(all(feature = "quic", not(target_arch = "wasm32")))]
pub use quic::{MAX_QUIC_FRAME_BYTES, METIS_QUIC_ALPN, QuicIpcTransport, quic_config};
pub use server::{IpcHandler, IpcServer};
#[cfg(not(target_arch = "wasm32"))]
pub use session_transport::NativeSessionTransport;
pub use transport::{AsyncIpcTransport, IpcTransport, MemoryTransport, StreamTransport};

#[cfg(target_arch = "wasm32")]
pub use browser::BrowserWebSocketTransport;
