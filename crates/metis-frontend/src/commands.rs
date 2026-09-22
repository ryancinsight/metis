//! Host-neutral command state for the authored application surface.

use crate::app::FrontendApp;
use metis_core::Result;
use metis_ipc::IpcTransport;

/// Presentation theme selected by an application command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ApplicationTheme {
    /// Use the native host's default light presentation.
    System,
    /// Use the bounded dark presentation palette.
    Dark,
}

impl ApplicationTheme {
    /// Default theme for a new frontend session.
    pub const DEFAULT: Self = Self::System;

    /// Returns the stable command status label for this theme.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::System => "system theme",
            Self::Dark => "dark theme",
        }
    }
}

impl Default for ApplicationTheme {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A local application command that does not cross the backend IPC boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ApplicationCommand {
    /// Move host focus to the authored patient reference control.
    FocusPatient,
    /// Apply the dark presentation palette.
    ThemeDark,
    /// Return to the system presentation palette.
    ThemeSystem,
}

impl ApplicationCommand {
    /// Returns the stable authored identity for this command.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::FocusPatient => "command-focus-patient",
            Self::ThemeDark => "command-theme-dark",
            Self::ThemeSystem => "command-theme-system",
        }
    }

    /// Resolves an authored command identity.
    #[must_use]
    pub const fn from_id(id: &str) -> Option<Self> {
        match id.as_bytes() {
            b"command-focus-patient" => Some(Self::FocusPatient),
            b"command-theme-dark" => Some(Self::ThemeDark),
            b"command-theme-system" => Some(Self::ThemeSystem),
            _ => None,
        }
    }

    /// Returns the live-region status after this command is applied.
    #[must_use]
    pub const fn status(self) -> &'static str {
        match self {
            Self::FocusPatient => "Patient reference focused",
            Self::ThemeDark => "Theme: dark",
            Self::ThemeSystem => "Theme: system",
        }
    }

    /// Returns the theme mutation carried by this command, if any.
    #[must_use]
    pub const fn theme(self) -> Option<ApplicationTheme> {
        match self {
            Self::FocusPatient => None,
            Self::ThemeDark => Some(ApplicationTheme::Dark),
            Self::ThemeSystem => Some(ApplicationTheme::System),
        }
    }

    /// Returns the browser control value carried by this command, if any.
    #[must_use]
    pub const fn theme_value(self) -> Option<&'static str> {
        match self.theme() {
            Some(ApplicationTheme::Dark) => Some("dark"),
            Some(ApplicationTheme::System) => Some("system"),
            None => None,
        }
    }
}

/// Visibility state for the local application command menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum CommandMenuState {
    /// The menu is excluded from layout and host actions.
    #[default]
    Closed,
    /// The menu is visible and its menu items may be activated.
    Open,
}

impl CommandMenuState {
    /// Returns whether the command menu is visible.
    #[must_use]
    pub const fn is_open(self) -> bool {
        matches!(self, Self::Open)
    }

    /// Returns the state after toggling visibility.
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            Self::Closed => Self::Open,
            Self::Open => Self::Closed,
        }
    }
}

impl<T: IpcTransport> FrontendApp<T> {
    /// Returns whether the local command menu is visible.
    #[must_use]
    pub const fn command_menu_open(&self) -> bool {
        self.command_menu.is_open()
    }

    /// Returns the current command live-region status.
    #[must_use]
    pub fn command_status(&self) -> &str {
        &self.command_status
    }

    /// Returns the current presentation theme.
    #[must_use]
    pub const fn theme(&self) -> ApplicationTheme {
        self.theme
    }

    /// Toggles the local command menu and repaints the authored surface.
    ///
    /// # Errors
    /// Preserves the prior menu state when the authored surface cannot render.
    pub fn toggle_command_menu(&mut self) -> Result<()> {
        let previous_menu = self.command_menu;
        let previous_status = self.command_status.clone();
        self.command_menu = self.command_menu.toggled();
        self.command_status = if self.command_menu_open() {
            "Commands opened".to_owned()
        } else {
            "Commands closed".to_owned()
        };
        if let Err(error) = self.render() {
            self.command_menu = previous_menu;
            self.command_status = previous_status;
            return Err(error);
        }
        Ok(())
    }

    /// Closes the local command menu when it is open.
    ///
    /// Returns whether a visible menu was closed.
    ///
    /// # Errors
    /// Preserves the prior menu state when the authored surface cannot render.
    pub fn close_command_menu(&mut self) -> Result<bool> {
        if !self.command_menu_open() {
            return Ok(false);
        }
        let previous_status = self.command_status.clone();
        self.command_menu = CommandMenuState::Closed;
        "Commands closed".clone_into(&mut self.command_status);
        if let Err(error) = self.render() {
            self.command_menu = CommandMenuState::Open;
            self.command_status = previous_status;
            return Err(error);
        }
        Ok(true)
    }

    /// Applies one local command and repaints the authored surface.
    ///
    /// # Errors
    /// Preserves the prior command state when the authored surface cannot render.
    pub fn activate_command(&mut self, command: ApplicationCommand) -> Result<()> {
        let previous_menu = self.command_menu;
        let previous_status = self.command_status.clone();
        let previous_theme = self.theme;
        self.command_menu = CommandMenuState::Closed;
        command.status().clone_into(&mut self.command_status);
        if let Some(theme) = command.theme() {
            self.theme = theme;
        }
        if let Err(error) = self.render() {
            self.command_menu = previous_menu;
            self.command_status = previous_status;
            self.theme = previous_theme;
            return Err(error);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ApplicationCommand, ApplicationTheme, CommandMenuState};
    use crate::app::FrontendApp;
    use metis_ipc::MemoryTransport;
    use metis_platform::Color;
    use metis_ui_lang::Display;

    #[test]
    fn command_actions_have_bounded_status_and_theme_semantics() {
        assert_eq!(
            ApplicationCommand::FocusPatient.status(),
            "Patient reference focused"
        );
        assert_eq!(
            ApplicationCommand::ThemeDark.theme(),
            Some(ApplicationTheme::Dark)
        );
        assert_eq!(
            ApplicationCommand::ThemeSystem.theme(),
            Some(ApplicationTheme::System)
        );
        assert_eq!(ApplicationCommand::FocusPatient.theme(), None);
        assert_eq!(
            ApplicationCommand::from_id("command-theme-dark"),
            Some(ApplicationCommand::ThemeDark)
        );
        assert_eq!(ApplicationCommand::from_id("command-shell"), None);
        assert_eq!(ApplicationCommand::ThemeSystem.id(), "command-theme-system");
        assert_eq!(ApplicationCommand::ThemeDark.theme_value(), Some("dark"));
    }

    #[test]
    fn menu_state_toggle_is_closed_under_repetition() {
        assert_eq!(CommandMenuState::Closed.toggled(), CommandMenuState::Open);
        assert_eq!(CommandMenuState::Open.toggled(), CommandMenuState::Closed);
        assert!(!CommandMenuState::Closed.is_open());
        assert!(CommandMenuState::Open.is_open());
    }

    #[test]
    fn frontend_commands_toggle_menu_apply_theme_and_preserve_status() {
        let (transport, _peer) = MemoryTransport::pair();
        let mut app = FrontendApp::new(transport, 800, 600).expect("initial form");
        assert_eq!(app.theme(), ApplicationTheme::System);
        assert!(!app.command_menu_open());
        assert_eq!(app.command_status(), "Commands ready");
        assert_eq!(
            app.document()
                .find_element_by_id("command-menu")
                .expect("command menu")
                .computed_style
                .display,
            Display::None
        );

        app.toggle_command_menu().expect("open command menu");
        assert!(app.command_menu_open());
        assert_eq!(app.command_status(), "Commands opened");
        assert_eq!(
            app.document()
                .find_element_by_id("command-menu-toggle")
                .expect("command toggle")
                .attributes
                .get("aria-expanded")
                .map(String::as_str),
            Some("true")
        );
        assert_eq!(
            app.document()
                .find_element_by_id("command-menu")
                .expect("command menu")
                .computed_style
                .display,
            Display::Flex
        );

        app.activate_command(ApplicationCommand::ThemeDark)
            .expect("dark theme");
        assert_eq!(app.theme(), ApplicationTheme::Dark);
        assert!(!app.command_menu_open());
        assert_eq!(app.command_status(), "Theme: dark");
        assert_eq!(app.framebuffer().get_pixel(0, 0), Color::rgb(15, 23, 42));

        app.activate_command(ApplicationCommand::ThemeSystem)
            .expect("system theme");
        assert_eq!(app.theme(), ApplicationTheme::System);
        assert_eq!(app.framebuffer().get_pixel(0, 0), Color::rgb(240, 244, 248));
        assert_eq!(app.command_status(), "Theme: system");
    }
}
