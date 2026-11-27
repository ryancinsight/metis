//! Ownership of the native message listener and the optional permission task.

use crate::message::PostedMessage;
use moirai_pal::wasm::{LocalTaskHandle, WebViewBridge, WebViewMessageListener};
use std::io;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{PageTransitionEvent, Window};

pub(super) fn connect(
    mut receive: impl FnMut(io::Result<PostedMessage>) + 'static,
) -> io::Result<(WebViewBridge, WebViewMessageListener)> {
    let bridge = WebViewBridge::current()?;
    let listener = bridge.listen(move |data| {
        receive(data.and_then(|text| serde_json::from_str(&text).map_err(io::Error::from)));
    })?;
    Ok((bridge, listener))
}

/// Render only the failure kind; a decoder error can include raw page input.
pub(super) fn reply_diagnostic(error: &io::Error) -> String {
    format!("Host reply could not be read ({:?}).", error.kind())
}

/// A cached document resumes with its original handlers. Leaving it for good
/// detaches the native callback and cancels any pending permission request.
pub(super) fn own_session(
    window: &Window,
    listener: WebViewMessageListener,
    probe: Option<LocalTaskHandle>,
) -> Result<(), wasm_bindgen::JsValue> {
    let mut resources = Some((listener, probe));
    let on_hide =
        Closure::<dyn FnMut(PageTransitionEvent)>::new(move |event: PageTransitionEvent| {
            if !event.persisted() {
                drop(resources.take());
            }
        });
    window.add_event_listener_with_callback("pagehide", on_hide.as_ref().unchecked_ref())?;
    on_hide.into_js_value();
    Ok(())
}
