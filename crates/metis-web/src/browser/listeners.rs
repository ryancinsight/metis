use super::{
    BrowserState, clipboard, commands, dialog, explorer, file_drop, fragment, gesture, pointer,
    text, view, wheel,
};
use crate::controls::{self, ControlField};
use crate::epoch::Generation;
use metis_frontend::AsyncFrontendApp;
use metis_ipc::BrowserWebSocketTransport;
use moirai_pal::wasm::{LocalTaskHandle, WebDocument, WebElement, WebEventListener};
use std::{
    cell::{Cell, RefCell},
    io,
    rc::Rc,
};

pub(super) struct ListenerContext<'a> {
    pub(super) document: &'a WebDocument,
    pub(super) state: &'a Rc<RefCell<BrowserState>>,
    pub(super) app: &'a Rc<RefCell<Option<AsyncFrontendApp<BrowserWebSocketTransport>>>>,
    pub(super) generation: Generation,
    pub(super) drop_task: &'a Rc<RefCell<Option<LocalTaskHandle>>>,
    pub(super) drop_sequence: &'a Rc<Cell<u64>>,
    pub(super) fragment_task: &'a Rc<RefCell<Option<LocalTaskHandle>>>,
    pub(super) clipboard_task: &'a Rc<RefCell<Option<LocalTaskHandle>>>,
}

pub(super) fn control_listeners(
    context: &ListenerContext<'_>,
) -> io::Result<Vec<WebEventListener>> {
    let ListenerContext {
        document,
        state,
        app,
        generation,
        drop_task,
        drop_sequence,
        fragment_task,
        clipboard_task,
    } = context;
    let root = view::element(document, "metis-app")?;
    let mut listeners = Vec::new();
    listeners.push(input_listener(document, state, app, &root)?);
    listeners.push(change_listener(document, state, &root)?);
    listeners.extend(commands::listeners(document, state)?);
    listeners.extend(dialog::listeners(document)?);
    listeners.extend(fragment::listeners(
        document,
        app,
        *generation,
        fragment_task,
    )?);
    listeners.extend(pointer::listeners(document)?);
    listeners.extend(wheel::listeners(document)?);
    listeners.extend(gesture::listeners(document)?);
    listeners.extend(file_drop::listeners(
        document,
        state,
        *generation,
        drop_task,
        drop_sequence,
    )?);
    listeners.extend(text::listeners(document, state)?);
    listeners.extend(clipboard::listeners(
        document,
        state,
        *generation,
        clipboard_task,
    )?);
    listeners.extend(explorer::listeners(document, state)?);
    Ok(listeners)
}

pub(super) fn input_listener(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    app: &Rc<RefCell<Option<AsyncFrontendApp<BrowserWebSocketTransport>>>>,
    root: &WebElement,
) -> io::Result<WebEventListener> {
    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    let listener_app = Rc::clone(app);
    root.add_event_listener("input", move |event| {
        let Some(target) = event.target() else {
            return;
        };
        let id = target.id();
        if let Some(field) = controls::input_field(&id) {
            let Some(value) = event.value() else {
                return;
            };
            let mut state = listener_state.borrow_mut();
            let BrowserState {
                inputs,
                state: form_state,
                ..
            } = &mut *state;
            controls::update_input(inputs, form_state, field, &value);
            if let Some(app) = listener_app.borrow_mut().as_mut() {
                let inputs = &state.inputs;
                app.set_inputs(
                    &inputs.patient_id,
                    inputs.weight_kg,
                    inputs.concentration_mg_ml,
                    inputs.target_dose_mcg_kg_min,
                );
            }
            if let Err(error) = view::render(&listener_document, &state) {
                view::set_mount_error(&listener_document, &error);
            }
            return;
        }
        let Some(field) = controls::input_control_field(&id) else {
            return;
        };
        apply_control_event(&listener_document, &listener_state, field, &event);
    })
}

pub(super) fn change_listener(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    root: &WebElement,
) -> io::Result<WebEventListener> {
    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    root.add_event_listener("change", move |event| {
        let Some(target) = event.target() else {
            return;
        };
        let Some(field) = controls::change_control_field(&target.id()) else {
            return;
        };
        apply_control_event(&listener_document, &listener_state, field, &event);
    })
}

pub(super) fn apply_control_event(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    field: ControlField,
    event: &moirai_pal::wasm::WebEvent,
) {
    let target = event.target();
    let checked = target.as_ref().and_then(WebElement::checked);
    let value = event.value();
    let mut state = state.borrow_mut();
    let BrowserState {
        controls,
        state: form_state,
        ..
    } = &mut *state;
    controls::update_control(controls, form_state, field, checked, value.as_deref());
    if let Err(error) = view::render(document, &state) {
        view::set_mount_error(document, &error);
    }
}
