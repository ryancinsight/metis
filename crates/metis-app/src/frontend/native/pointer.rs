//! Pointer presses on the native form: what a press lands on, and its action.

use super::NativeForm;
use super::keyboard::PATIENT_INPUT;
use metis_core::error::Result;
use metis_frontend::{ApplicationCommand, FocusOrigin};
use metis_ipc::IpcTransport;

/// Controls a press can hit while the command menu is closed, in hit order.
const FORM_TARGETS: [&str; 3] = [
    "command-menu-toggle",
    ApplicationCommand::FocusPatient.id(),
    "btn-calc",
];
/// Controls a press can hit while the command menu is open, in hit order.
const MENU_TARGETS: [&str; 3] = [
    "command-menu-toggle",
    ApplicationCommand::ThemeDark.id(),
    ApplicationCommand::ThemeSystem.id(),
];

/// What a pointer press lands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PointerHit {
    /// A control the press focuses and activates.
    Control(&'static str),
    /// The open command menu between its items.
    MenuSurface,
    /// Anything but the open command menu and its items; it dismisses the menu.
    OutsideMenu,
    /// The patient reference while the menu is closed.
    PatientInput,
    /// Nothing the press acts on.
    Nothing,
}

impl<T: IpcTransport> NativeForm<T> {
    /// Applies the action of what a press at `(x, y)` lands on, returning
    /// whether the frame changed.
    pub(super) fn handle_pointer_up(&mut self, x: i32, y: i32) -> Result<bool> {
        self.focused = true;
        match self.pointer_hit(x, y)? {
            PointerHit::Control(target) => {
                // A press focuses what it hits without a ring, then acts.
                self.app.focus_control(target, FocusOrigin::Pointer)?;
                self.activate_control(target, FocusOrigin::Pointer)
            }
            PointerHit::OutsideMenu => self.app.close_command_menu(),
            PointerHit::PatientInput => self.app.focus_control(PATIENT_INPUT, FocusOrigin::Pointer),
            PointerHit::MenuSurface | PointerHit::Nothing => Ok(false),
        }
    }

    /// What a press at `(x, y)` lands on in the painted frame.
    pub(super) fn pointer_hit(&self, x: i32, y: i32) -> Result<PointerHit> {
        let menu_open = self.app.command_menu_open();
        let targets = if menu_open {
            MENU_TARGETS
        } else {
            FORM_TARGETS
        };
        for target in targets {
            if self.app.element_rect(target)?.contains(x, y) {
                return Ok(PointerHit::Control(target));
            }
        }
        Ok(if menu_open {
            if self.app.element_rect("command-menu")?.contains(x, y) {
                PointerHit::MenuSurface
            } else {
                PointerHit::OutsideMenu
            }
        } else if self.app.element_rect(PATIENT_INPUT)?.contains(x, y) {
            PointerHit::PatientInput
        } else {
            PointerHit::Nothing
        })
    }
}
