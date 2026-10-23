//! Command surface rendering for the form's toolbar and menu.

use crate::document::TrackedDocument;
use crate::{FormState, FrontendApp};
use metis_core::{ErrorCode, MetisError, Result};
use metis_ui_lang::Display;

/// Reports an edit that found no element as a malformed authored form.
pub(crate) fn found(edit: metis_ui_lang::Edit, missing: impl FnOnce() -> String) -> Result<()> {
    if edit == metis_ui_lang::Edit::Missing {
        Err(MetisError::ui(ErrorCode::MalformedMarkup, missing()))
    } else {
        Ok(())
    }
}

fn set_text(doc: &mut TrackedDocument, id: &str, text: &str) -> Result<()> {
    found(doc.set_text(id, text), || {
        format!("Authored form is missing label {id}")
    })
}

fn set_attribute(doc: &mut TrackedDocument, id: &str, key: &str, value: &str) -> Result<()> {
    found(doc.set_attribute(id, key, value), || {
        format!("Authored form is missing semantic field {id}")
    })
}

fn bool_text(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

impl<T: metis_ipc::IpcTransport> FrontendApp<T> {
    /// Renders the command toolbar and menu surface.
    pub(crate) fn render_command_surface(&mut self) -> Result<()> {
        let menu_open = self.command_menu_open();
        let doc = &mut self.doc;
        set_attribute(
            doc,
            "command-menu-toggle",
            "aria-expanded",
            bool_text(menu_open),
        )?;
        set_attribute(doc, "command-menu", "aria-hidden", bool_text(!menu_open))?;
        let display = if menu_open {
            Display::Flex
        } else {
            Display::None
        };
        found(
            doc.restyle("command-menu", |style| style.display = display),
            || "Authored form is missing command menu".to_owned(),
        )?;
        set_text(doc, "command-status", &self.command_status)
    }
}