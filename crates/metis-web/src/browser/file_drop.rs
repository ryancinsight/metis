//! Browser file-drop listeners and rendering transitions.

use super::BrowserState;
use super::view;
use crate::file_drop_policy::{DropState, FileDropEntry, FileDropError};
use moirai_pal::wasm::{WebDocument, WebElement, WebEventListener};
use std::cell::RefCell;
use std::io;
use std::rc::Rc;

pub(super) fn listeners(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
) -> io::Result<Vec<WebEventListener>> {
    let zone = view::element(document, "drop-zone")?;
    let status = view::element(document, "drop-status")?;
    let mut listeners = Vec::with_capacity(4);

    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    let listener_zone = zone.clone();
    listeners.push(zone.add_event_listener("dragenter", move |event| {
        event.prevent_default();
        update_state(
            &listener_document,
            &listener_state,
            &listener_zone,
            DropState::hovering(),
        );
    })?);

    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    let listener_zone = zone.clone();
    listeners.push(zone.add_event_listener("dragover", move |event| {
        event.prevent_default();
        if !matches!(listener_state.borrow().drop_state, DropState::Hovering) {
            update_state(
                &listener_document,
                &listener_state,
                &listener_zone,
                DropState::hovering(),
            );
        }
    })?);

    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    let listener_zone = zone.clone();
    listeners.push(zone.add_event_listener("dragleave", move |event| {
        event.prevent_default();
        update_state(
            &listener_document,
            &listener_state,
            &listener_zone,
            DropState::default(),
        );
    })?);

    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    let listener_status = status.clone();
    listeners.push(zone.add_event_listener("drop", move |event| {
        event.prevent_default();
        handle_drop(
            &listener_document,
            &listener_state,
            &listener_status,
            &event,
        );
    })?);

    Ok(listeners)
}

fn handle_drop(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    status: &WebElement,
    event: &moirai_pal::wasm::WebEvent,
) {
    let metadata = match event.drop_metadata() {
        Ok(Some(metadata)) => metadata,
        Ok(None) => {
            update_rejection(
                document,
                state,
                status,
                FileDropError::ProviderMetadata,
                None,
            );
            return;
        }
        Err(error) => {
            update_rejection(
                document,
                state,
                status,
                FileDropError::ProviderMetadata,
                Some(error.to_string()),
            );
            return;
        }
    };
    let entries = metadata.files().iter().map(|file| {
        FileDropEntry::new(
            file.name().to_owned(),
            file.media_type().to_owned(),
            file.size_bytes(),
        )
    });
    let accepted = entries
        .collect::<Result<Vec<_>, _>>()
        .and_then(DropState::accept);
    match accepted {
        Ok(next_state) => update_state(document, state, status, next_state),
        Err(error) => update_rejection(document, state, status, error, None),
    }
}

fn update_state(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    zone: &WebElement,
    next_state: DropState,
) {
    state.borrow_mut().drop_state = next_state;
    if let Err(error) = view::render(document, &state.borrow()) {
        view::set_status_error(document, zone, "File drop", &error.to_string());
    }
}

fn update_rejection(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    status: &WebElement,
    error: FileDropError,
    provider_message: Option<String>,
) {
    state.borrow_mut().drop_state = DropState::Rejected(error);
    if let Err(render_error) = view::render(document, &state.borrow()) {
        view::set_status_error(document, status, "File drop", &render_error.to_string());
        return;
    }
    if let Some(message) = provider_message {
        view::set_status_error(document, status, "File drop", &message);
    }
}
