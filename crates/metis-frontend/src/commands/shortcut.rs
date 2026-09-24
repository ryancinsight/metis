//! Keyboard accelerators for the local application commands.
//!
//! Every host resolves its key events to an [`Accelerator`] and asks this
//! module for the command, so one declaration serves the native window, the
//! browser workbench and the announced `aria-keyshortcuts` value. The
//! bindings use Alt+Shift because browsers reserve most Control chords
//! (Ctrl+P prints, Ctrl+Shift+D bookmarks) before a page can see them.

use super::ApplicationCommand;
use crate::app::FrontendApp;
use metis_core::Result;
use metis_core::input::{Accelerator, Key, Letter, Modifiers};
use metis_ipc::IpcTransport;

/// Builds an Alt+Shift+letter accelerator; a non-letter fails compilation
/// because every call site is a constant.
const fn alt_shift(byte: u8) -> Accelerator {
    let Some(letter) = Letter::new(byte) else {
        panic!("command shortcuts bind ASCII letters");
    };
    Accelerator::new(Modifiers::ALT.union(Modifiers::SHIFT), Key::Letter(letter))
}

impl ApplicationCommand {
    /// Every local command, in menu order.
    pub const ALL: [Self; 3] = [Self::FocusPatient, Self::ThemeDark, Self::ThemeSystem];

    /// The keyboard accelerator bound to this command.
    #[must_use]
    pub const fn accelerator(self) -> Accelerator {
        match self {
            Self::FocusPatient => alt_shift(b'P'),
            Self::ThemeDark => alt_shift(b'D'),
            Self::ThemeSystem => alt_shift(b'S'),
        }
    }

    /// Resolves a pressed accelerator to its command.
    #[must_use]
    pub fn from_accelerator(accelerator: Accelerator) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|command| command.accelerator() == accelerator)
    }
}

impl<T: IpcTransport> FrontendApp<T> {
    /// Activates the command bound to `accelerator`, returning whether one
    /// was bound. Unbound accelerators leave the surface untouched so the host
    /// can apply its own default.
    ///
    /// # Errors
    /// Preserves the prior command state when the authored surface cannot render.
    pub fn activate_shortcut(&mut self, accelerator: Accelerator) -> Result<bool> {
        let Some(command) = ApplicationCommand::from_accelerator(accelerator) else {
            return Ok(false);
        };
        self.activate_command(command)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::ApplicationCommand;
    use crate::{CLINICAL_SCREEN_XML, FrontendApp};
    use metis_core::input::{Accelerator, Modifiers, ShortcutMap};
    use metis_ipc::MemoryTransport;

    #[test]
    fn accelerators_are_unique_and_resolve_to_their_command() {
        let mut map = ShortcutMap::new();
        for command in ApplicationCommand::ALL {
            map.bind(command.accelerator(), command)
                .expect("command accelerators must not conflict");
            assert_eq!(
                ApplicationCommand::from_accelerator(command.accelerator()),
                Some(command)
            );
        }
        assert_eq!(
            ApplicationCommand::ThemeDark.accelerator().to_string(),
            "Alt+Shift+D"
        );
        let unbound =
            Accelerator::parse_with_primary("CmdOrCtrl+D", Modifiers::CTRL).expect("accelerator");
        assert_eq!(ApplicationCommand::from_accelerator(unbound), None);
    }

    #[test]
    fn authored_markup_announces_each_command_shortcut() {
        let (transport, _peer) = MemoryTransport::pair();
        let app = FrontendApp::new(transport, 800, 600).expect("initial form");
        for command in ApplicationCommand::ALL {
            let element = app
                .document()
                .find_element_by_id(command.id())
                .expect("authored command");
            assert_eq!(
                element.attributes.get("aria-keyshortcuts"),
                Some(&command.accelerator().aria_keyshortcuts()),
                "{} in CLINICAL_SCREEN_XML",
                command.id()
            );
        }
        assert!(CLINICAL_SCREEN_XML.contains("aria-keyshortcuts"));
    }

    #[test]
    fn shortcuts_activate_commands_and_ignore_unbound_chords() {
        let (transport, _peer) = MemoryTransport::pair();
        let mut app = FrontendApp::new(transport, 800, 600).expect("initial form");
        assert!(
            app.activate_shortcut(ApplicationCommand::ThemeDark.accelerator())
                .expect("dark shortcut")
        );
        assert_eq!(app.command_status(), "Theme: dark");
        let unbound = Accelerator::parse("Alt+Shift+Q").expect("accelerator");
        assert!(!app.activate_shortcut(unbound).expect("unbound shortcut"));
        assert_eq!(app.command_status(), "Theme: dark");
    }
}
