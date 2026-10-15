//! The calculation form: element handles, the command menu contract and
//! the form runner wiring commands, shortcuts, submit and typed replies.

use super::commands::install_form_commands;
use super::submit::wire_form_bridge;
use super::{apply_theme_if_known, requested_theme};
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, HtmlElement, Window};

/// One frontend element handle bundle for the calculation form.
#[derive(Clone)]
pub(super) struct FormElements {
    /// The whole calculation form.
    pub(super) form: Element,
    /// The bounded theme select.
    pub(super) theme_mode: web_sys::HtmlSelectElement,
    /// The patient reference input.
    pub(super) patient: web_sys::HtmlInputElement,
    /// The weight input, in kilograms.
    pub(super) weight: web_sys::HtmlInputElement,
    /// The concentration input, in mg/mL.
    pub(super) concentration: web_sys::HtmlInputElement,
    /// The target dose input, in mcg/kg/min.
    pub(super) dose: web_sys::HtmlInputElement,
    /// The host connection status line.
    pub(super) status: Element,
    /// The result line.
    pub(super) result: Element,
    /// The command menu toggle.
    pub(super) menu_toggle: HtmlElement,
    /// The command menu.
    pub(super) menu: Element,
    /// The command status line.
    pub(super) command_status: Element,
    /// The only submit button inside the form.
    pub(super) submit: web_sys::HtmlButtonElement,
    /// The focus-patient command button.
    pub(super) focus_patient: HtmlElement,
    /// The dark-theme menu command button.
    pub(super) theme_dark: HtmlElement,
    /// The system-theme menu command button.
    pub(super) theme_system: HtmlElement,
    /// The copy-result menu command button.
    pub(super) copy_result: HtmlElement,
}

/// Casts one element id through its concrete DOM interface.
fn element_of<T: JsCast>(document: &Document, id: &str) -> Option<T> {
    document.get_element_by_id(id)?.dyn_into::<T>().ok()
}

impl FormElements {
    fn collect(document: &Document) -> Option<Self> {
        let form = document.get_element_by_id("calculation")?;
        let submit = form
            .query_selector("button")
            .ok()
            .flatten()?
            .dyn_into::<web_sys::HtmlButtonElement>()
            .ok()?;
        Some(Self {
            form,
            theme_mode: element_of(document, "theme-mode")?,
            patient: element_of(document, "patient-id")?,
            weight: element_of(document, "weight")?,
            concentration: element_of(document, "concentration")?,
            dose: element_of(document, "dose")?,
            status: document.get_element_by_id("host-status")?,
            result: document.get_element_by_id("result")?,
            menu_toggle: element_of(document, "command-menu-toggle")?,
            menu: document.get_element_by_id("command-menu")?,
            command_status: document.get_element_by_id("command-status")?,
            submit,
            focus_patient: element_of(document, "command-focus-patient")?,
            theme_dark: element_of(document, "command-theme-dark")?,
            theme_system: element_of(document, "command-theme-system")?,
            copy_result: element_of(document, "command-copy-result")?,
        })
    }

    /// Shows one line in the result region and re-enables submission.
    pub(super) fn show_error(&self, line: &str) {
        self.result.set_text_content(Some(line));
        self.submit.set_disabled(false);
    }
}

/// Opens or closes the command menu with the retired focus contract: focus
/// returns to the toggle only when a visible menu closes, and a shortcut
/// applied with the menu closed leaves focus where the user was.
pub(super) fn set_command_menu(elements: &FormElements, open: bool, message: &str) {
    let was_open = elements
        .menu_toggle
        .get_attribute("aria-expanded")
        .as_deref()
        == Some("true");
    let _ignored = elements
        .menu_toggle
        .set_attribute("aria-expanded", if open { "true" } else { "false" });
    let _ignored = elements
        .menu
        .set_attribute("aria-hidden", if open { "false" } else { "true" });
    let _ignored = elements.menu.set_attribute(
        "data-command-menu-open",
        if open { "true" } else { "false" },
    );
    elements.command_status.set_text_content(Some(message));
    if open {
        if let Ok(Some(item)) = elements.menu.query_selector("[role='menuitem']") {
            let _ignored = item.unchecked_ref::<HtmlElement>().focus();
        }
    } else if was_open {
        let _ignored = elements.menu_toggle.focus();
    }
}

/// Runs the packaged calculation form: theme, commands, shortcuts, submit and
/// typed replies, with the same user-visible lines and focus contract as the
/// retired script.
pub(super) fn run_form(window: &Window, document: &Document) {
    let Some(elements) = FormElements::collect(document) else {
        return;
    };
    if let Some(theme) = requested_theme(window, document) {
        elements.theme_mode.set_value(theme.attribute());
    }
    apply_theme_if_known(document, &elements.theme_mode.value());
    install_form_commands(document, &elements);
    wire_form_bridge(window, &elements);
}
