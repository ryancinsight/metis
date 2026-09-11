#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

pub mod audit;
pub mod clinical;
mod fragment;
#[cfg(not(target_arch = "wasm32"))]
mod http;
mod plugins;
pub mod service;
pub mod supervisor;
#[cfg(not(target_arch = "wasm32"))]
pub mod websocket;

pub use fragment::UiFragmentPlugin;
#[cfg(not(target_arch = "wasm32"))]
pub use http::{BrowserHttpService, MAX_HTTP_REQUESTS, MAX_HTTP_SESSIONS, serve_browser_http};
pub use plugins::PluginExecutor;
pub use service::BackendService;
pub use supervisor::{
    INTERACTIVE_SESSION_DEADLINE, ProcessEnvironment, SESSION_DEADLINE, run_session,
    run_session_with_deadline, run_session_with_deadline_and_environment,
};
#[cfg(not(target_arch = "wasm32"))]
pub use websocket::{serve_browser_websocket, serve_browser_websocket_with_response_delay};
