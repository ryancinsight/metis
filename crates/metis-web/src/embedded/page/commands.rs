//! Authored command identities and physical shortcuts share one bounded catalogue.

use super::{
    application::State,
    coordinator::{Coordinator, invalid},
    view,
};
use crate::{
    Theme,
    embedded::{PageEvent, PageRole},
};
use metis_core::input::{Accelerator, Key, Modifiers};
use metis_frontend::{ApplicationCommand, CommandMenuState};
use moirai_pal::wasm::{KeyboardMetadata, WebEventListener};
use std::{cell::RefCell, io, rc::Rc};

/// A remount owns new state while retaining the document and patient draft.
pub(super) fn initialize(page: &Coordinator, state: &Rc<RefCell<State>>) -> io::Result<()> {
    menu(
        page,
        state,
        CommandMenuState::Closed,
        "Commands ready",
        false,
    )
}

pub(super) fn close_listener(
    page: &Rc<Coordinator>,
    state: &Rc<RefCell<State>>,
) -> io::Result<Vec<WebEventListener>> {
    let weak = Rc::downgrade(page);
    let state = Rc::clone(state);
    Ok(vec![page.document.add_event_listener(
        "keydown",
        move |event| {
            let Some(page) = weak.upgrade() else {
                return;
            };
            let result = (|| {
                page.require_ready(page.identity())?;
                let Some(keyboard) = event.keyboard_metadata()? else {
                    return Ok(());
                };
                if keyboard.is_repeat() {
                    return Ok(());
                }
                if keyboard.key() == "Escape" {
                    event.prevent_default();
                    if state.borrow().menu.is_open() {
                        menu(
                            &page,
                            &state,
                            CommandMenuState::Closed,
                            "Commands closed",
                            true,
                        )
                    } else {
                        page.post(&PageEvent::CloseRequested {
                            identity: page.identity(),
                        })
                    }
                } else if page.identity().role() == PageRole::Form
                    && let Some(command) =
                        accelerator(&keyboard).and_then(ApplicationCommand::from_accelerator)
                {
                    event.prevent_default();
                    activate(&page, &state, command)
                } else {
                    Ok(())
                }
            })();
            if let Err(error) = result {
                page.failure(&error);
            }
        },
    )?])
}

pub(super) fn listeners(
    page: &Rc<Coordinator>,
    state: &Rc<RefCell<State>>,
) -> io::Result<Vec<WebEventListener>> {
    let mut listeners = Vec::new();
    let weak = Rc::downgrade(page);
    let menu_state = Rc::clone(state);
    listeners.push(
        view::element(&page.document, "command-menu-toggle")?.add_event_listener(
            "click",
            move |event| {
                event.prevent_default();
                let Some(page) = weak.upgrade() else {
                    return;
                };
                let result = page.require_ready(page.identity()).and_then(|()| {
                    let target = menu_state.borrow().menu.toggled();
                    let message = if target.is_open() {
                        "Commands opened"
                    } else {
                        "Commands closed"
                    };
                    menu(&page, &menu_state, target, message, true)
                });
                if let Err(error) = result {
                    page.failure(&error);
                }
            },
        )?,
    );
    for command in ApplicationCommand::ALL {
        let weak = Rc::downgrade(page);
        let state = Rc::clone(state);
        listeners.push(
            view::element(&page.document, command.id())?.add_event_listener(
                "click",
                move |event| {
                    event.prevent_default();
                    let Some(page) = weak.upgrade() else {
                        return;
                    };
                    let result = page
                        .require_ready(page.identity())
                        .and_then(|()| activate(&page, &state, command));
                    if let Err(error) = result {
                        page.failure(&error);
                    }
                },
            )?,
        );
    }
    let weak = Rc::downgrade(page);
    listeners.push(
        view::element(&page.document, "theme-mode")?.add_event_listener(
            "change",
            move |event| {
                let Some(page) = weak.upgrade() else {
                    return;
                };
                let result = page.require_ready(page.identity()).and_then(|()| {
                    let theme = event
                        .value()
                        .as_deref()
                        .and_then(Theme::parse)
                        .ok_or_else(|| invalid("Theme change has no declared value"))?;
                    view::apply_theme(&page.document, theme)
                });
                if let Err(error) = result {
                    page.failure(&error);
                }
            },
        )?,
    );
    Ok(listeners)
}

fn accelerator(metadata: &KeyboardMetadata) -> Option<Accelerator> {
    // Physical codes preserve the authored shortcut across keyboard layouts.
    let key = Key::from_browser_code(metadata.code())?;
    let held = metadata.modifiers();
    let modifiers = Modifiers::NONE
        .with(Modifiers::CTRL, held.ctrl())
        .with(Modifiers::ALT, held.alt())
        .with(Modifiers::SHIFT, held.shift())
        .with(Modifiers::META, held.meta());
    Some(Accelerator::new(modifiers, key))
}

fn activate(
    page: &Coordinator,
    state: &Rc<RefCell<State>>,
    command: ApplicationCommand,
) -> io::Result<()> {
    if let Some(value) = command.theme_value() {
        view::apply_theme(
            &page.document,
            Theme::parse(value)
                .ok_or_else(|| invalid("Command theme is outside the declared domain"))?,
        )?;
    }
    let status = if let Some(value) = command.theme_value() {
        format!(
            "Theme: {}",
            Theme::parse(value)
                .ok_or_else(|| invalid("Command theme is undeclared"))?
                .label()
        )
    } else {
        command.status().to_owned()
    };
    menu(page, state, CommandMenuState::Closed, &status, true)?;
    if command == ApplicationCommand::FocusPatient {
        view::element(&page.document, "patient-id")?.focus()?;
    }
    Ok(())
}

fn menu(
    page: &Coordinator,
    state: &Rc<RefCell<State>>,
    target: CommandMenuState,
    message: &str,
    restore_focus: bool,
) -> io::Result<()> {
    let was_open = state.borrow().menu.is_open();
    let open = target.is_open();
    let toggle = view::element(&page.document, "command-menu-toggle")?;
    let menu = view::element(&page.document, "command-menu")?;
    toggle.set_attribute("aria-expanded", if open { "true" } else { "false" })?;
    menu.set_attribute("aria-hidden", if open { "false" } else { "true" })?;
    menu.set_attribute(
        "data-command-menu-open",
        if open { "true" } else { "false" },
    )?;
    view::element(&page.document, "command-status")?.set_text(message);
    state.borrow_mut().menu = target;
    if open {
        view::element(&page.document, ApplicationCommand::ThemeDark.id())?.focus()?;
    } else if restore_focus && was_open {
        toggle.focus()?;
    }
    Ok(())
}
