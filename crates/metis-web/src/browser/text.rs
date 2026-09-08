//! Browser text-control listeners backed by the Rust editing policy.

use super::{BrowserState, view};
use crate::text_policy::{
    CompositionPhase, CompositionState, Selection, SelectionDirection, TextError,
};
use moirai_pal::wasm::{TextSelectionDirection, WebDocument, WebElement, WebEventListener};
use std::cell::RefCell;
use std::io;
use std::rc::Rc;

pub(super) fn listeners(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
) -> io::Result<Vec<WebEventListener>> {
    let control = view::element(document, "text-specimen")?;
    let status = view::element(document, "text-status")?;
    let mut listeners = Vec::with_capacity(6);
    listeners.push(input_listener(document, state, &control, &status)?);
    listeners.extend(composition_listeners(document, state, &control, &status)?);
    listeners.push(selection_listener(document, state, &control, &status)?);
    Ok(listeners)
}

fn input_listener(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    control: &WebElement,
    status: &WebElement,
) -> io::Result<WebEventListener> {
    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    let listener_status = status.clone();
    control.add_event_listener("input", move |event| {
        let metadata = match event.text_input_metadata() {
            Ok(Some(metadata)) => metadata,
            Ok(None) => {
                report_error(
                    &listener_document,
                    &listener_status,
                    "Text input event did not target a text control",
                );
                return;
            }
            Err(error) => {
                report_error(&listener_document, &listener_status, &error.to_string());
                return;
            }
        };
        let selection = match selection_from_provider(metadata.selection()) {
            Ok(selection) => selection,
            Err(error) => {
                report_error(&listener_document, &listener_status, &error.to_string());
                return;
            }
        };
        let value = metadata.value().to_owned();
        let data = metadata.data().map(str::to_owned);
        let input_type = metadata.input_type().to_owned();
        let composition = if metadata.is_composing() {
            CompositionState::Active
        } else {
            CompositionState::Inactive
        };
        let result = listener_state.borrow_mut().text_state.apply_input(
            value,
            data,
            input_type,
            composition,
            selection,
        );
        if let Err(error) = result {
            report_error(&listener_document, &listener_status, &error.to_string());
            return;
        }
        render(&listener_document, &listener_state, &listener_status);
    })
}

fn composition_listeners(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    control: &WebElement,
    status: &WebElement,
) -> io::Result<Vec<WebEventListener>> {
    let mut listeners = Vec::with_capacity(4);
    for (event_name, phase) in [
        ("compositionstart", CompositionPhase::Start),
        ("compositionupdate", CompositionPhase::Update),
        ("compositionend", CompositionPhase::Commit),
        ("compositioncancel", CompositionPhase::Cancel),
    ] {
        let listener_document = document.clone();
        let listener_state = Rc::clone(state);
        let listener_status = status.clone();
        listeners.push(control.add_event_listener(event_name, move |event| {
            let metadata = match event.composition_metadata() {
                Ok(Some(metadata)) => metadata,
                Ok(None) => {
                    report_error(
                        &listener_document,
                        &listener_status,
                        "Composition event did not carry metadata",
                    );
                    return;
                }
                Err(error) => {
                    report_error(&listener_document, &listener_status, &error.to_string());
                    return;
                }
            };
            let result = listener_state.borrow_mut().text_state.apply_composition(
                phase,
                metadata.data().map(str::to_owned),
                metadata.locale().to_owned(),
            );
            if let Err(error) = result {
                report_error(&listener_document, &listener_status, &error.to_string());
                return;
            }
            render(&listener_document, &listener_state, &listener_status);
        })?);
    }
    Ok(listeners)
}

fn selection_listener(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    control: &WebElement,
    status: &WebElement,
) -> io::Result<WebEventListener> {
    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    let listener_status = status.clone();
    control.add_event_listener("select", move |event| {
        let Some(target) = event.target() else {
            report_error(
                &listener_document,
                &listener_status,
                "Selection event did not carry a target",
            );
            return;
        };
        let selection = match target.text_selection() {
            Ok(Some(selection)) => selection,
            Ok(None) => {
                report_error(
                    &listener_document,
                    &listener_status,
                    "Selection event did not target a text control",
                );
                return;
            }
            Err(error) => {
                report_error(&listener_document, &listener_status, &error.to_string());
                return;
            }
        };
        let selection = match selection_from_provider(selection) {
            Ok(selection) => selection,
            Err(error) => {
                report_error(&listener_document, &listener_status, &error.to_string());
                return;
            }
        };
        let result = listener_state
            .borrow_mut()
            .text_state
            .apply_selection(selection);
        if let Err(error) = result {
            report_error(&listener_document, &listener_status, &error.to_string());
            return;
        }
        render(&listener_document, &listener_state, &listener_status);
    })
}

fn selection_from_provider(
    selection: moirai_pal::wasm::TextSelection,
) -> Result<Selection, TextError> {
    let direction = match selection.direction() {
        TextSelectionDirection::Forward => SelectionDirection::Forward,
        TextSelectionDirection::Backward => SelectionDirection::Backward,
        TextSelectionDirection::None => SelectionDirection::None,
        _ => SelectionDirection::Other,
    };
    Selection::new(selection.start(), selection.end(), direction)
}

fn render(document: &WebDocument, state: &Rc<RefCell<BrowserState>>, status: &WebElement) {
    if let Err(error) = view::render(document, &state.borrow()) {
        view::set_status_error(document, status, "Text input", &error.to_string());
    }
}

fn report_error(document: &WebDocument, status: &WebElement, message: &str) {
    view::set_status_error(document, status, "Text input", message);
}
