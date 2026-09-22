use super::{BrowserState, view};
use crate::controls::{self, ControlField};
use moirai_pal::wasm::{WebDocument, WebEventListener};
use std::{cell::RefCell, io, rc::Rc};

/// Rust-owned state for the browser command surface.
pub(super) struct CommandState {
    pub(super) menu_open: bool,
    pub(super) status: String,
}

impl Default for CommandState {
    fn default() -> Self {
        Self {
            menu_open: false,
            status: "Commands ready".to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommandAction {
    FocusPatient,
    ThemeDark,
    ThemeSystem,
}

impl CommandAction {
    fn from_id(id: &str) -> Option<Self> {
        match id {
            "command-focus-patient" => Some(Self::FocusPatient),
            "command-theme-dark" => Some(Self::ThemeDark),
            "command-theme-system" => Some(Self::ThemeSystem),
            _ => None,
        }
    }

    const fn status(self) -> &'static str {
        match self {
            Self::FocusPatient => "Command applied: patient reference focused",
            Self::ThemeDark => "Command applied: dark theme",
            Self::ThemeSystem => "Command applied: system theme",
        }
    }

    const fn theme_value(self) -> Option<&'static str> {
        match self {
            Self::ThemeDark => Some("dark"),
            Self::ThemeSystem => Some("system"),
            Self::FocusPatient => None,
        }
    }
}

pub(super) fn listeners(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
) -> io::Result<Vec<WebEventListener>> {
    let toggle = view::element(document, "command-menu-toggle")?;
    let menu = view::element(document, "command-menu")?;
    Ok(vec![
        toggle_listener(document, state, &toggle)?,
        action_listener(
            document,
            state,
            &view::element(document, "command-focus-patient")?,
        )?,
        action_listener(
            document,
            state,
            &view::element(document, "command-theme-dark")?,
        )?,
        action_listener(
            document,
            state,
            &view::element(document, "command-theme-system")?,
        )?,
        escape_listener(document, state, &menu, &toggle)?,
    ])
}

fn toggle_listener(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    toggle: &moirai_pal::wasm::WebElement,
) -> io::Result<WebEventListener> {
    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    toggle.add_event_listener("click", move |event| {
        event.prevent_default();
        let mut state = listener_state.borrow_mut();
        state.commands.menu_open = !state.commands.menu_open;
        if state.commands.menu_open {
            "Commands menu open".clone_into(&mut state.commands.status);
        } else {
            "Commands menu closed".clone_into(&mut state.commands.status);
        }
        if let Err(error) = view::render(&listener_document, &state) {
            view::set_mount_error(&listener_document, &error);
        }
    })
}

fn action_listener(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    control: &moirai_pal::wasm::WebElement,
) -> io::Result<WebEventListener> {
    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    control.add_event_listener("click", move |event| {
        event.prevent_default();
        let Some(action) = event
            .target()
            .and_then(|target| CommandAction::from_id(&target.id()))
        else {
            return;
        };
        let result = (|| -> io::Result<()> {
            let should_focus = {
                let mut state = listener_state.borrow_mut();
                if let Some(theme) = action.theme_value() {
                    let BrowserState {
                        controls,
                        state: form_state,
                        ..
                    } = &mut *state;
                    controls::update_control(
                        controls,
                        form_state,
                        ControlField::Theme,
                        None,
                        Some(theme),
                    );
                    view::element(&listener_document, "theme-mode")?.set_value(theme)?;
                }
                state.commands.menu_open = false;
                action.status().clone_into(&mut state.commands.status);
                view::render(&listener_document, &state)?;
                action == CommandAction::FocusPatient
            };
            if should_focus {
                view::element(&listener_document, "patient-id")?.focus()?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            view::set_mount_error(&listener_document, &error);
        }
    })
}

fn escape_listener(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    menu: &moirai_pal::wasm::WebElement,
    toggle: &moirai_pal::wasm::WebElement,
) -> io::Result<WebEventListener> {
    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    let listener_toggle = toggle.clone();
    menu.add_event_listener("keydown", move |event| {
        let metadata = match event.keyboard_metadata() {
            Ok(Some(metadata)) => metadata,
            Ok(None) => return,
            Err(error) => {
                view::set_mount_error(&listener_document, &error);
                return;
            }
        };
        if metadata.key() != "Escape" {
            return;
        }
        event.prevent_default();
        let result = {
            let mut state = listener_state.borrow_mut();
            state.commands.menu_open = false;
            "Commands menu closed".clone_into(&mut state.commands.status);
            view::render(&listener_document, &state)
        };
        if let Err(error) = result {
            view::set_mount_error(&listener_document, &error);
            return;
        }
        if let Err(error) = listener_toggle.focus() {
            view::set_mount_error(&listener_document, &error);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::CommandAction;

    #[test]
    fn command_ids_map_to_bounded_actions() {
        assert_eq!(
            CommandAction::from_id("command-focus-patient"),
            Some(CommandAction::FocusPatient)
        );
        assert_eq!(
            CommandAction::from_id("command-theme-dark"),
            Some(CommandAction::ThemeDark)
        );
        assert_eq!(
            CommandAction::from_id("command-theme-system"),
            Some(CommandAction::ThemeSystem)
        );
        assert_eq!(CommandAction::from_id("command-shell"), None);
    }

    #[test]
    fn theme_commands_carry_the_control_value() {
        assert_eq!(CommandAction::ThemeDark.theme_value(), Some("dark"));
        assert_eq!(CommandAction::ThemeSystem.theme_value(), Some("system"));
        assert_eq!(CommandAction::FocusPatient.theme_value(), None);
    }
}
