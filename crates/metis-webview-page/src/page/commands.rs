//! The local command surface: theme select, command menu, page commands,
//! announced keyboard shortcuts and their retired focus contract.

use super::form::{FormElements, set_command_menu};
use super::shortcuts::install_shortcuts;
use super::{apply_theme_if_known, retain};
use wasm_bindgen::{closure::Closure, prelude::*};
use web_sys::Document;

/// Installs the local command surface: theme select, command menu, page
/// commands, announced keyboard shortcuts and their retired focus contract.
pub(super) fn install_form_commands(document: &Document, elements: &FormElements) {
    install_theme_select(document, elements);
    install_menu_toggle(elements);
    install_menu_escape(elements);
    install_focus_patient(elements);
    install_theme_buttons(document, elements);
    install_copy_command(elements);
    install_shortcuts(document);
}

/// Applies the admitted theme whenever the bounded theme select changes.
fn install_theme_select(document: &Document, elements: &FormElements) {
    let change_document = document.clone();
    let change_elements = elements.clone();
    let on_change = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
        apply_theme_if_known(&change_document, &change_elements.theme_mode.value());
    });
    let _ignored = elements
        .theme_mode
        .add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref());
    retain(on_change);
}

/// Toggles the command menu from its button, announcing the new state.
fn install_menu_toggle(elements: &FormElements) {
    let toggle_elements = elements.clone();
    let on_toggle = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
        let open = toggle_elements
            .menu_toggle
            .get_attribute("aria-expanded")
            .as_deref()
            != Some("true");
        let message = if open {
            "Commands opened"
        } else {
            "Commands closed"
        };
        set_command_menu(&toggle_elements, open, message);
    });
    let _ignored = elements
        .menu_toggle
        .add_event_listener_with_callback("click", on_toggle.as_ref().unchecked_ref());
    retain(on_toggle);
}

/// Closes the command menu on Escape with the retired focus contract.
fn install_menu_escape(elements: &FormElements) {
    let escape_elements = elements.clone();
    let on_escape = Closure::<dyn FnMut(JsValue)>::new(move |value: JsValue| {
        (|| {
            let Some(event) = value.dyn_ref::<web_sys::KeyboardEvent>() else {
                return;
            };
            if event.key() != "Escape" {
                return;
            }
            event.prevent_default();
            set_command_menu(&escape_elements, false, "Commands closed");
        })();
    });
    let _ignored = elements
        .menu
        .add_event_listener_with_callback("keydown", on_escape.as_ref().unchecked_ref());
    retain(on_escape);
}

/// Focuses the patient reference from its menu command, closing the menu.
fn install_focus_patient(elements: &FormElements) {
    let focus_elements = elements.clone();
    let on_focus_patient = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
        set_command_menu(&focus_elements, false, "Patient reference focused");
        let _ignored = focus_elements.patient.focus();
    });
    let _ignored = elements
        .focus_patient
        .add_event_listener_with_callback("click", on_focus_patient.as_ref().unchecked_ref());
    retain(on_focus_patient);
}

/// Applies the dark and system themes from their menu commands.
fn install_theme_buttons(document: &Document, elements: &FormElements) {
    let dark_elements = elements.clone();
    let dark_document = document.clone();
    let on_theme_dark = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
        dark_elements.theme_mode.set_value("dark");
        apply_theme_if_known(&dark_document, "dark");
        set_command_menu(&dark_elements, false, "Theme: dark");
    });
    let _ignored = elements
        .theme_dark
        .add_event_listener_with_callback("click", on_theme_dark.as_ref().unchecked_ref());
    retain(on_theme_dark);

    let system_elements = elements.clone();
    let system_document = document.clone();
    let on_theme_system = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
        system_elements.theme_mode.set_value("system");
        apply_theme_if_known(&system_document, "system");
        set_command_menu(&system_elements, false, "Theme: system preference");
    });
    let _ignored = elements
        .theme_system
        .add_event_listener_with_callback("click", on_theme_system.as_ref().unchecked_ref());
    retain(on_theme_system);
}

/// Copies the visible result line through the page-local browser clipboard:
/// no bridge authority is needed to carry the user's own visible result text
/// back to the same display surface.
fn install_copy_command(elements: &FormElements) {
    let copy_elements = elements.clone();
    let on_copy = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
        let Some(text) = copy_elements.result.text_content() else {
            copy_elements
                .command_status
                .set_text_content(Some("No result to copy"));
            return;
        };
        let text = text.trim().to_owned();
        if text.is_empty()
            || text.starts_with("Rate: No result")
            || text.starts_with("Submitting to the supervised backend")
        {
            copy_elements
                .command_status
                .set_text_content(Some("No result to copy"));
            return;
        }
        let Some(window) = web_sys::window() else {
            copy_elements
                .command_status
                .set_text_content(Some("Copy failed (no window context)"));
            return;
        };
        let clipboard = window.navigator().clipboard();
        let write = clipboard.write_text(&text);
        let fulfilled_elements = copy_elements.clone();
        let on_fulfilled = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
            fulfilled_elements
                .command_status
                .set_text_content(Some("Result copied to clipboard"));
        });
        let rejected_elements = copy_elements.clone();
        let on_rejected = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
            rejected_elements
                .command_status
                .set_text_content(Some("Copy failed (clipboard denied)"));
        });
        // A `Closure` is the `'static` spelling of the binder's scoped
        // closures, so the promise chain accepts both references directly.
        let _ignored = write.then(&on_fulfilled).catch(&on_rejected);
        retain(on_fulfilled);
        retain(on_rejected);
    });
    let _ignored = elements
        .copy_result
        .add_event_listener_with_callback("click", on_copy.as_ref().unchecked_ref());
    retain(on_copy);
}
