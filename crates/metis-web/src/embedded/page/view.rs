//! DOM operations use typed PAL handles and preserve untrusted values as text.

use super::coordinator::{Coordinator, invalid};
use crate::{Theme, embedded::PageRole};
use metis_frontend::ApplicationCommand;
use moirai_pal::wasm::{WebDocument, WebElement, WebLocation};
use std::io;

pub(super) fn element(document: &WebDocument, id: &str) -> io::Result<WebElement> {
    document
        .get_element_by_id(id)
        .ok_or_else(|| invalid(format!("Packaged page has no {id}")))
}

pub(super) fn validate(page: &Coordinator) -> io::Result<()> {
    for id in ["host-status", "result"] {
        element(&page.document, id)?;
    }
    match page.identity().role() {
        PageRole::Form => {
            for id in [
                "calculation",
                "patient-id",
                "weight",
                "concentration",
                "dose",
                "calculation-submit",
                "theme-mode",
                "theme-state",
                "command-menu-toggle",
                "command-menu",
                "command-status",
            ] {
                element(&page.document, id)?;
            }
            for command in ApplicationCommand::ALL {
                let control = element(&page.document, command.id())?;
                if control.attribute("aria-keyshortcuts").as_deref()
                    != Some(command.accelerator().aria_keyshortcuts().as_str())
                {
                    return Err(invalid(format!(
                        "Command {} has inconsistent shortcut metadata",
                        command.id()
                    )));
                }
            }
        }
        PageRole::PermissionProbe => {
            for kind in crate::embedded::PageCapability::ALL {
                element(&page.document, &format!("permission-{}", kind.as_str()))?;
            }
        }
    }
    Ok(())
}

pub(super) fn configure_theme(document: &WebDocument) -> io::Result<()> {
    let requested = super::super::theme::requested(
        WebLocation::current()?.parameter("theme", super::super::theme::QUERY_BYTES),
    )?;
    let authored = document
        .body()?
        .attribute("data-metis-theme")
        .ok_or_else(|| invalid("Page theme is absent"))?;
    let theme = requested
        .or_else(|| Theme::parse(&authored))
        .ok_or_else(|| invalid("Page theme is undeclared"))?;
    apply_theme(document, theme)
}

pub(super) fn apply_theme(document: &WebDocument, theme: Theme) -> io::Result<()> {
    document
        .body()?
        .set_attribute("data-metis-theme", theme.css_value())?;
    element(document, "theme-mode")?.set_value(theme.css_value())?;
    element(document, "theme-state")?.set_text(&format!("Theme: {}", theme.label()));
    Ok(())
}

pub(super) fn failure(document: &WebDocument, message: &str) {
    if let Ok(result) = element(document, "result") {
        result.set_text(message);
    }
    if let Ok(status) = element(document, "host-status") {
        status.set_text("Rust page operation failed");
    }
}
