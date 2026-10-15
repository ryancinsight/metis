//! One application owns its listeners and its actual scheduled request task.

use super::{commands, coordinator::Coordinator, form, view};
use crate::embedded::{CapabilityObservation, PageRole, SequenceId};
use metis_frontend::CommandMenuState;
use moirai_pal::wasm::{LocalTaskHandle, WebEventListener};
use std::{cell::RefCell, io, rc::Rc};

pub(super) struct State {
    pub(super) menu: CommandMenuState,
    pub(super) next_request: Option<SequenceId>,
    pub(super) observations: [Option<CapabilityObservation>; 4],
    pub(super) denials: [Option<bool>; 4],
    pub(super) probe_completed: bool,
    pub(super) final_completion_published: bool,
}

pub(super) struct Application {
    pub(super) state: Rc<RefCell<State>>,
    listeners: Vec<WebEventListener>,
    pub(super) task: Option<LocalTaskHandle>,
}

impl Application {
    pub(super) fn mount(page: &Rc<Coordinator>) -> io::Result<Self> {
        view::validate(page)?;
        let state = Rc::new(RefCell::new(State {
            menu: CommandMenuState::Closed,
            next_request: Some(SequenceId::new(1)?),
            observations: std::array::from_fn(|_| None),
            denials: [None; 4],
            probe_completed: false,
            final_completion_published: false,
        }));
        let mut listeners = commands::keyboard::listeners(page, &state)?;
        if page.identity().role() == PageRole::Form {
            view::configure_theme(&page.document)?;
            commands::initialize(page, &state)?;
            form::initialize(page)?;
            listeners.extend(commands::listeners(page, &state)?);
            listeners.push(form::listener(page, &state)?);
        }
        Ok(Self {
            state,
            listeners,
            task: None,
        })
    }

    pub(super) fn close(&mut self) -> io::Result<()> {
        self.task.take();
        let mut first_error = None;
        for listener in &mut self.listeners {
            if let Err(error) = listener.close() {
                first_error.get_or_insert(error);
            }
        }
        self.listeners.clear();
        first_error.map_or(Ok(()), Err)
    }
}
