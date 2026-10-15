use super::{BrowserState, view};
use crate::controls::{self, ControlField};
use metis_core::input::{Accelerator, Key, Modifiers};
use metis_frontend::ApplicationCommand;
use moirai_pal::wasm::{KeyboardMetadata, LocalTaskHandle, WebDocument, WebEventListener};
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

pub(super) fn listeners(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    clipboard_task: &Rc<RefCell<Option<LocalTaskHandle>>>,
    generation: crate::epoch::Generation,
) -> io::Result<Vec<WebEventListener>> {
    let toggle = view::element(document, "command-menu-toggle")?;
    let menu = view::element(document, "command-menu")?;
    Ok(vec![
        toggle_listener(document, state, &toggle)?,
        // The copy-result command closes below via its own async flow.,
        action_listener(
            document,
            state,
            clipboard_task,
            generation,
            &view::element(document, "command-focus-patient")?,
        )?,
        action_listener(
            document,
            state,
            clipboard_task,
            generation,
            &view::element(document, "command-theme-dark")?,
        )?,
        action_listener(
            document,
            state,
            clipboard_task,
            generation,
            &view::element(document, "command-theme-system")?,
        )?,
        action_listener(
            document,
            state,
            clipboard_task,
            generation,
            &view::element(document, "command-copy-result")?,
        )?,
        escape_listener(document, state, &menu, &toggle)?,
        shortcut_listener(document, state, clipboard_task, generation)?,
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
        if let Err(error) = view::render(&listener_document, &mut state) {
            view::set_mount_error(&listener_document, &error);
        }
    })
}

fn action_listener(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    clipboard_task: &Rc<RefCell<Option<LocalTaskHandle>>>,
    generation: crate::epoch::Generation,
    control: &moirai_pal::wasm::WebElement,
) -> io::Result<WebEventListener> {
    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    let listener_task = Rc::clone(clipboard_task);
    let listener_generation = generation;
    control.add_event_listener("click", move |event| {
        event.prevent_default();
        let Some(command) = event
            .target()
            .and_then(|target| ApplicationCommand::from_id(&target.id()))
        else {
            return;
        };
        if let Err(error) = apply(
            &listener_document,
            &listener_state,
            command,
            &listener_task,
            listener_generation,
        ) {
            view::set_mount_error(&listener_document, &error);
        }
    })
}

/// Resolves keyboard accelerators anywhere in the page to the same commands
/// the menu activates. Auto-repeat is ignored so a held chord applies once,
/// and unbound chords keep the browser's default action.
fn shortcut_listener(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    clipboard_task: &Rc<RefCell<Option<LocalTaskHandle>>>,
    generation: crate::epoch::Generation,
) -> io::Result<WebEventListener> {
    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    let listener_task = Rc::clone(clipboard_task);
    let listener_generation = generation;
    document
        .body()?
        .add_event_listener("keydown", move |event| {
            let metadata = match event.keyboard_metadata() {
                Ok(Some(metadata)) => metadata,
                Ok(None) => return,
                Err(error) => {
                    view::set_mount_error(&listener_document, &error);
                    return;
                }
            };
            if metadata.is_repeat() {
                return;
            }
            let Some(command) =
                accelerator(&metadata).and_then(ApplicationCommand::from_accelerator)
            else {
                return;
            };
            event.prevent_default();
            if let Err(error) = apply(
                &listener_document,
                &listener_state,
                command,
                &listener_task,
                listener_generation,
            ) {
                view::set_mount_error(&listener_document, &error);
            }
        })
}

/// Builds the pressed accelerator, preferring the layout-independent
/// physical code and falling back to the produced key value.
fn accelerator(metadata: &KeyboardMetadata) -> Option<Accelerator> {
    let key = Key::from_browser_code(metadata.code())
        .or_else(|| Key::from_browser_key(metadata.key()))?;
    let held = metadata.modifiers();
    let modifiers = Modifiers::NONE
        .with(Modifiers::CTRL, held.ctrl())
        .with(Modifiers::ALT, held.alt())
        .with(Modifiers::SHIFT, held.shift())
        .with(Modifiers::META, held.meta());
    Some(Accelerator::new(modifiers, key))
}

/// Applies one command: theme commands update the bound control, every
/// command closes the menu and reports its status, and the focus command
/// moves focus after the render so the new frame is what receives it.
fn apply(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    command: ApplicationCommand,
    clipboard_task: &Rc<RefCell<Option<LocalTaskHandle>>>,
    generation: crate::epoch::Generation,
) -> io::Result<()> {
    if command == ApplicationCommand::CopyResult {
        state.borrow_mut().commands.menu_open = false;
        // The async copy flow owns the command status line; the sync path
        // here never invents an outcome the clipboard has not reported yet.
        return super::clipboard::copy_result_command(document, state, clipboard_task, generation);
    }
    {
        let mut state = state.borrow_mut();
        if let Some(value) = command.theme_value() {
            let BrowserState {
                controls,
                state: form_state,
                ..
            } = &mut *state;
            controls::update_control(controls, form_state, ControlField::Theme, None, Some(value));
            view::element(document, "theme-mode")?.set_value(value)?;
        }
        state.commands.menu_open = false;
        command.status().clone_into(&mut state.commands.status);
        view::render(document, &mut state)?;
    }
    if command == ApplicationCommand::FocusPatient {
        view::element(document, "patient-id")?.focus()?;
    }
    Ok(())
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
            view::render(&listener_document, &mut state)
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
    use metis_frontend::ApplicationCommand;

    #[test]
    fn command_ids_map_to_bounded_actions() {
        assert_eq!(
            ApplicationCommand::from_id("command-focus-patient"),
            Some(ApplicationCommand::FocusPatient)
        );
        assert_eq!(
            ApplicationCommand::from_id("command-theme-dark"),
            Some(ApplicationCommand::ThemeDark)
        );
        assert_eq!(
            ApplicationCommand::from_id("command-theme-system"),
            Some(ApplicationCommand::ThemeSystem)
        );
        assert_eq!(ApplicationCommand::from_id("command-shell"), None);
    }

    #[test]
    fn theme_commands_carry_the_control_value() {
        assert_eq!(ApplicationCommand::ThemeDark.theme_value(), Some("dark"));
        assert_eq!(
            ApplicationCommand::ThemeSystem.theme_value(),
            Some("system")
        );
        assert_eq!(ApplicationCommand::FocusPatient.theme_value(), None);
    }
}
