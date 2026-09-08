#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

pub mod audit;
pub mod clinical;
mod plugins;
pub mod service;
pub mod supervisor;
#[cfg(not(target_arch = "wasm32"))]
pub mod websocket;

pub use plugins::PluginExecutor;
pub use service::BackendService;
pub use supervisor::{
    INTERACTIVE_SESSION_DEADLINE, SESSION_DEADLINE, run_session, run_session_with_deadline,
};
#[cfg(not(target_arch = "wasm32"))]
pub use websocket::{serve_browser_websocket, serve_browser_websocket_with_response_delay};
