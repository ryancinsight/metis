//! One keyboard listener preserves readiness, metadata, command and result order.

use super::super::{
    application::State, command_observation as observation, coordinator::Coordinator,
};
use super::{activate, menu};
use crate::embedded::{
    PageEvent, PageRole,
    command_observation::{Admission, Decision, Outcome},
};
use metis_core::input::{Accelerator, Key, Modifiers};
use metis_frontend::{ApplicationCommand, CommandMenuState};
use moirai_pal::wasm::{KeyboardMetadata, WebEvent, WebEventListener};
use std::{cell::RefCell, io, rc::Rc};

pub(in crate::embedded::page) fn listeners(
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
            handle(&page, &state, &event);
        },
    )?])
}

fn handle(page: &Coordinator, state: &Rc<RefCell<State>>, event: &WebEvent) {
    let mut invocation = observation::entered(page);
    let result = read_and_dispatch(page, state, event, &mut invocation);
    observation::returned(page, invocation, &result);
    if let Err(error) = result {
        page.failure(&error);
    }
}

fn read_and_dispatch(
    page: &Coordinator,
    state: &Rc<RefCell<State>>,
    event: &WebEvent,
    invocation: &mut Option<observation::Invocation>,
) -> io::Result<()> {
    if let Err(error) = page.require_ready(page.identity()) {
        observation::metadata(
            page,
            invocation,
            None,
            Admission::NotReady,
            Decision::NotEvaluated,
        );
        return Err(error);
    }
    observation::phase(invocation, Outcome::MetadataError);
    let keyboard = match event.keyboard_metadata() {
        Ok(keyboard) => keyboard,
        Err(error) => {
            observation::metadata(
                page,
                invocation,
                None,
                Admission::MetadataError,
                Decision::NotEvaluated,
            );
            return Err(error);
        }
    };
    let Some(keyboard) = keyboard else {
        observation::metadata(
            page,
            invocation,
            None,
            Admission::NotKeyboard,
            Decision::NotEvaluated,
        );
        return Ok(());
    };
    if keyboard.is_repeat() {
        observation::metadata(
            page,
            invocation,
            Some(&keyboard),
            Admission::Ready,
            Decision::Repeat,
        );
        return Ok(());
    }
    observation::phase(invocation, Outcome::ActionError);
    dispatch(page, state, event, &keyboard, invocation)
}

fn dispatch(
    page: &Coordinator,
    state: &Rc<RefCell<State>>,
    event: &WebEvent,
    keyboard: &KeyboardMetadata,
    invocation: &mut Option<observation::Invocation>,
) -> io::Result<()> {
    if keyboard.key() == "Escape" {
        event.prevent_default();
        let is_open = state.borrow().menu.is_open();
        if is_open {
            observation::metadata(
                page,
                invocation,
                Some(keyboard),
                Admission::Ready,
                Decision::EscapeMenu,
            );
            menu(
                page,
                state,
                CommandMenuState::Closed,
                "Commands closed",
                true,
            )
        } else {
            observation::metadata(
                page,
                invocation,
                Some(keyboard),
                Admission::Ready,
                Decision::EscapeClose,
            );
            // Closing intent precedes the original functional
            // request. No diagnostic reply follows it into Stopping.
            observation::closing(page, invocation);
            page.post(&PageEvent::CloseRequested {
                identity: page.identity(),
            })
        }
    } else if page.identity().role() == PageRole::Form {
        let command = accelerator(keyboard).and_then(ApplicationCommand::from_accelerator);
        if let Some(command) = command {
            observation::metadata(
                page,
                invocation,
                Some(keyboard),
                Admission::Ready,
                Decision::Command(command),
            );
            event.prevent_default();
            activate(page, state, command)
        } else {
            observation::metadata(
                page,
                invocation,
                Some(keyboard),
                Admission::Ready,
                Decision::Ignored,
            );
            Ok(())
        }
    } else {
        observation::metadata(
            page,
            invocation,
            Some(keyboard),
            Admission::Ready,
            Decision::Ignored,
        );
        Ok(())
    }
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
