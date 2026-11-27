//! Ownership of the native message listener and the optional permission task.

use crate::message::PostedMessage;
use moirai_pal::wasm::{LocalTaskHandle, WebViewBridge, WebViewMessageListener};
use std::io;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{PageTransitionEvent, Window};

pub(super) fn connect(
    mut receive: impl FnMut(PostedMessage) + 'static,
) -> io::Result<(WebViewBridge, WebViewMessageListener)> {
    let bridge = WebViewBridge::current()?;
    let listener = bridge.listen(move |data| {
        if let Ok(text) = data
            && let Ok(message) = serde_json::from_str(&text)
        {
            receive(message);
        }
    })?;
    Ok((bridge, listener))
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
