//! Copy-result menu-item accessibility for the native form.
//!
//! A closed menu refuses the activation, matching the theme items; an open
//! menu activates the shared copy command through the same menu contract.

use super::NativeForm;
use metis_core::error::Result;
use metis_frontend::ApplicationCommand;
use metis_ipc::IpcTransport;
use metis_platform::native::{AccessibilityAction, AccessibilityActionRequest};

/// Applies one copy-result accessibility request against `form`.
pub(super) fn apply<T: IpcTransport>(
    form: &mut NativeForm<T>,
    request: &AccessibilityActionRequest,
) -> Result<bool> {
    if !form.app.command_menu_open() {
        return Ok(false);
    }
    match request.action {
        AccessibilityAction::Activate => {
            form.app.activate_command(ApplicationCommand::CopyResult)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
