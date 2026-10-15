//! Announced-shortcut dispatch: each command's binding is read from its own
//! `aria-keyshortcuts` at press time, so the binding announced to assistive
//! technology is the one that runs.

use super::retain;
use crate::logic::{KeyPress, shortcut_text};
use wasm_bindgen::{JsCast, closure::Closure, prelude::*};
use web_sys::{Element, EventTarget, HtmlElement};

/// Reads one keyboard event through the pure shortcut vocabulary contract.
fn pressed_shortcut(event: &web_sys::KeyboardEvent) -> Option<String> {
    shortcut_text(&KeyPress {
        code: event.code(),
        control: event.ctrl_key(),
        alt: event.alt_key(),
        shift: event.shift_key(),
        meta: event.meta_key(),
    })
}

/// Runs the announced-shortcut dispatcher: each command's binding is read
/// from its own `aria-keyshortcuts` at press time, so the binding announced
/// to assistive technology is the one that runs.
pub(super) fn install_shortcuts(target: &EventTarget) {
    let listener = Closure::<dyn FnMut(JsValue)>::new(move |value: JsValue| {
        (|| {
            let Some(event) = value.dyn_ref::<web_sys::KeyboardEvent>() else {
                return;
            };
            if event.repeat() {
                return;
            }
            let Some(pressed) = pressed_shortcut(event) else {
                return;
            };
            let Some(document) = web_sys::window().and_then(|window| window.document()) else {
                return;
            };
            let Ok(candidates) = document.query_selector_all("button[aria-keyshortcuts]") else {
                return;
            };
            for index in 0..candidates.length() {
                let Some(item) = candidates
                    .item(index)
                    .and_then(|node| node.dyn_into::<Element>().ok())
                else {
                    continue;
                };
                if item.get_attribute("aria-keyshortcuts").as_deref() != Some(pressed.as_str()) {
                    continue;
                }
                event.prevent_default();
                item.unchecked_ref::<HtmlElement>().click();
                return;
            }
        })();
    });
    let _ignored =
        target.add_event_listener_with_callback("keydown", listener.as_ref().unchecked_ref());
    retain(listener);
}
