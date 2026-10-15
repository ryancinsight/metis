//! The typed host bridge: `window.chrome.webview` reached with structural
//! browser bindings, exactly like the retired truthiness checks.

use js_sys::{Function, Reflect as JsReflect};
use wasm_bindgen::{JsCast, closure::Closure, prelude::*};
use web_sys::Window;

/// Reads `window.chrome.webview` when the packaged host bound it, exactly like
/// the retired truthiness check.
pub(super) fn find_bridge(window: &Window) -> Option<JsValue> {
    let chrome = JsReflect::get(&window.clone().into(), &JsValue::from_str("chrome")).ok()?;
    if chrome.is_null() || chrome.is_undefined() {
        return None;
    }
    let webview = JsReflect::get(&chrome, &JsValue::from_str("webview")).ok()?;
    if webview.is_null() || webview.is_undefined() {
        return None;
    }
    Some(webview)
}

/// Posts one JSON text payload through the typed host bridge.
pub(super) fn post_text(bridge: &JsValue, payload: &str) {
    let Ok(method) = JsReflect::get(bridge, &JsValue::from_str("postMessage")) else {
        return;
    };
    let Ok(function) = method.dyn_into::<Function>() else {
        return;
    };
    let _ignored = function.call1(bridge, &JsValue::from_str(payload));
}

/// Registers one bridge message listener that lives for the page lifetime.
pub(super) fn listen_for_messages(bridge: &JsValue, listener: &Closure<dyn FnMut(JsValue)>) {
    let Ok(method) = JsReflect::get(bridge, &JsValue::from_str("addEventListener")) else {
        return;
    };
    let Ok(function) = method.dyn_into::<Function>() else {
        return;
    };
    let _ignored = function.call2(
        bridge,
        &JsValue::from_str("message"),
        listener.as_ref().unchecked_ref(),
    );
}

/// Reads a bridge message's `data` entry the way the retired script did: only
/// string payloads carry a typed envelope.
pub(super) fn message_data(event: &JsValue) -> Option<String> {
    let data = JsReflect::get(event, &JsValue::from_str("data")).ok()?;
    data.as_string()
}
