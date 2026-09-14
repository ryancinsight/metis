//! Windows-native window and `WebView2` adapters backed by Moirai.

mod application;
mod dialog;
mod webview;
mod window;

pub use application::{NativeApplication, NativeFlow, NativeHostError, run_native_application};
pub use dialog::{DialogSelection, MAX_DIALOG_PATH_UNITS, pick};

pub use webview::{
    MAX_WEBVIEW_EVENTS, MAX_WEBVIEW_MESSAGE_BYTES, MAX_WEBVIEW_MESSAGE_UNITS,
    MAX_WEBVIEW_URI_UNITS, MAX_WEBVIEW_WAIT_MILLISECONDS, WebViewConfig, WebViewEvent,
    WebViewHostEvent, WebViewSurface,
};
pub use window::{
    CompositionPhase, MAX_COMPOSITION_UNITS, MAX_FRAME_DIMENSION, MAX_FRAME_PIXELS,
    MAX_PUMP_MESSAGES, MAX_TITLE_UNITS, MAX_WAIT_MILLISECONDS, MAX_WINDOW_EVENTS, ModifierState,
    MouseButton, NativeSurface, WindowConfig, WindowEvent, WindowVisibility,
};
