//! Keyboard focus navigation and activation for the native form.
//!
//! Tab and Shift+Tab move focus through the frontend's navigation order.
//! Enter and Space activate the focused control, following the button
//! pattern; the patient reference keeps Enter as the submit shortcut and
//! Space as text. Keyboard activation of the menu button opens the menu with
//! focus on its first item, as the menu-button pattern does.

use super::{NativeForm, submit};
use metis_core::error::Result;
use metis_core::input::{Accelerator, Key, Modifiers};
use metis_frontend::{ApplicationCommand, FocusDirection, FocusOrigin};
use metis_ipc::IpcTransport;
use metis_platform::native::ModifierState;

const TAB_KEY: u32 = 0x09;
const SPACE_KEY: u32 = 0x20;
const RETURN_KEY: u32 = super::RETURN_KEY;
/// Authored id of the patient reference, the form's one text control.
pub(super) const PATIENT_INPUT: &str = "label-patient";
/// Authored id of the command-menu button.
const MENU_BUTTON: &str = "command-menu-toggle";
/// Authored id of the submit control.
const SUBMIT: &str = "btn-calc";

/// The accelerator a key press forms, or `None` for keys outside the
/// shared vocabulary, such as a bare modifier.
pub(super) fn accelerator(virtual_key: u32, held: ModifierState) -> Option<Accelerator> {
    let key = Key::from_windows_virtual_key(virtual_key)?;
    let modifiers = Modifiers::NONE
        .with(Modifiers::CTRL, held.ctrl())
        .with(Modifiers::ALT, held.alt())
        .with(Modifiers::SHIFT, held.shift())
        .with(Modifiers::META, held.meta());
    Some(Accelerator::new(modifiers, key))
}

impl<T: IpcTransport> NativeForm<T> {
    /// Handles the keys this module owns: a first press of a command
    /// accelerator, then focus keys. Returns whether the frame changed, or
    /// `None` for a key that other handlers own.
    pub(super) fn handle_key_down(
        &mut self,
        virtual_key: u32,
        repeated: bool,
        modifiers: ModifierState,
    ) -> Result<Option<bool>> {
        if !repeated
            && let Some(accelerator) = accelerator(virtual_key, modifiers)
            && self.app.activate_shortcut(accelerator)?
        {
            return Ok(Some(true));
        }
        self.handle_focus_key(virtual_key, repeated, modifiers)
    }

    /// Handles a focus key, returning whether the frame changed, or `None`
    /// for a key that other handlers own.
    fn handle_focus_key(
        &mut self,
        virtual_key: u32,
        repeated: bool,
        modifiers: ModifierState,
    ) -> Result<Option<bool>> {
        if modifiers.ctrl() || modifiers.alt() || modifiers.meta() {
            return Ok(None);
        }
        match virtual_key {
            TAB_KEY => {
                let direction = if modifiers.shift() {
                    FocusDirection::Backward
                } else {
                    FocusDirection::Forward
                };
                self.app.move_focus(direction)?;
                Ok(Some(true))
            }
            RETURN_KEY | SPACE_KEY if !repeated && !modifiers.shift() => {
                if self.app.focused_control() == PATIENT_INPUT {
                    return Ok(None);
                }
                let control = self.app.focused_control().to_owned();
                self.activate_control(&control, FocusOrigin::Keyboard)
                    .map(Some)
            }
            _ => Ok(None),
        }
    }

    /// Performs the action of the control `id`, returning whether the frame
    /// changed; `origin` decides whether a menu opened here takes focus.
    pub(super) fn activate_control(&mut self, id: &str, origin: FocusOrigin) -> Result<bool> {
        if id == MENU_BUTTON {
            self.app.toggle_command_menu()?;
            if self.app.command_menu_open() && origin == FocusOrigin::Keyboard {
                // The dark-theme command is the menu's first authored item.
                self.app
                    .focus_control(ApplicationCommand::ThemeDark.id(), FocusOrigin::Keyboard)?;
            }
            return Ok(true);
        }
        if let Some(command) = ApplicationCommand::from_id(id) {
            self.app.activate_command(command)?;
            return Ok(true);
        }
        if id == SUBMIT {
            submit(&mut self.app, self.pid)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Whether typed text belongs to the patient reference: the window has
    /// focus and so does the control.
    pub(super) fn patient_has_focus(&self) -> bool {
        self.focused && self.app.focused_control() == PATIENT_INPUT
    }
}

#[cfg(test)]
#[path = "keyboard_tests.rs"]
mod tests;
