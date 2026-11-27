//! The page presentation: one Rust lifecycle for the packaged `WebView2`
//! form and permission-probe pages.
//!
//! The module mirrors the retired embedded scripts behaviorally: same element
//! ids, same user-visible text, same event wiring and the same typed bridge
//! payload and reply handling. The host bridge (`window.chrome.webview`) is
//! reached with structural browser bindings, host replies are typed Rust
//! values parsed from JSON, and unknown or malformed payloads fail closed:
//! they change no page state. Page listeners live for the document lifetime
//! and disappear with surface teardown, which is exactly the detachment
//! contract the owning item accepts.

use crate::message::PostedMessage;
use wasm_bindgen::{closure::Closure, prelude::*};
use web_sys::{Document, Window};

use crate::logic::Theme;

mod bridge;
mod commands;
mod form;
mod probe;
mod shortcuts;
mod submit;

/// Runs the packaged page once the module is instantiated. The page kind is
/// the presence of the probe's fixed result list.
pub fn start() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    if document.get_element_by_id("permission-results").is_some() {
        probe::run_permission_probe(&window, &document);
    } else {
        form::run_form(&window, &document);
    }
}

/// Reads the page's requested theme with the retired script's fallback order:
/// the first `?theme=` entry wins, then the document's declared attribute.
/// Unknown values return nothing, exactly like the retired map lookup guard.
pub(super) fn requested_theme(window: &Window, document: &Document) -> Option<Theme> {
    let search = window.location().search().ok()?;
    for entry in search.trim_start_matches('?').split('&') {
        if let Some(("theme", value)) = entry.split_once('=') {
            return Theme::parse(value);
        }
    }
    let declared = document.body()?.get_attribute("data-metis-theme")?;
    Theme::parse(&declared)
}

/// Applies one theme: the body attribute plus its rendered description.
fn apply_theme(document: &Document, theme: Theme) {
    if let Some(body) = document.body() {
        let _ignored = body.set_attribute("data-metis-theme", theme.attribute());
    }
    let description = format!("Theme: {}", theme.label());
    if let Some(state) = document.get_element_by_id("theme-state") {
        state.set_text_content(Some(&description));
    }
}

/// Applies a theme only when the value is one of the admitted page themes,
/// matching the retired label-map guard that ignored unknown values.
pub(super) fn apply_theme_if_known(document: &Document, value: &str) {
    if let Some(theme) = Theme::parse(value) {
        apply_theme(document, theme);
    }
}

/// Stores one listener registration for the page lifetime: the document owns
/// the closure until surface teardown destroys the document itself.
pub(super) fn retain(listener: Closure<dyn FnMut(JsValue)>) {
    Box::leak(Box::new(listener));
}
