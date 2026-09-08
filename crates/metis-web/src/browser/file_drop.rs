//! Browser file-drop listeners and rendering transitions.

use super::BrowserState;
use super::generation_is_current;
use super::view;
use crate::epoch::Generation;
use crate::file_drop_policy::{
    DicomHeader, DropReadState, DropState, FileDropEntry, FileDropError, classify_dicom_header,
};
use moirai_pal::wasm::{
    DropFiles, LocalTaskHandle, WebDocument, WebElement, WebEventListener, spawn_local_with_handle,
};
use std::cell::{Cell, RefCell};
use std::io;
use std::rc::Rc;

struct DropReadContext {
    document: WebDocument,
    state: Rc<RefCell<BrowserState>>,
    status: WebElement,
    generation: Generation,
    task_slot: Rc<RefCell<Option<LocalTaskHandle>>>,
    sequence: Rc<Cell<u64>>,
}

pub(super) fn listeners(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    generation: Generation,
    drop_task: &Rc<RefCell<Option<LocalTaskHandle>>>,
    drop_sequence: &Rc<Cell<u64>>,
) -> io::Result<Vec<WebEventListener>> {
    let zone = view::element(document, "drop-zone")?;
    let status = view::element(document, "drop-byte-status")?;
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
    let listener_drop_task = Rc::clone(drop_task);
    let listener_drop_sequence = Rc::clone(drop_sequence);
    listeners.push(zone.add_event_listener("drop", move |event| {
        event.prevent_default();
        handle_drop(
            &listener_document,
            &listener_state,
            &listener_status,
            &event,
            generation,
            &listener_drop_task,
            &listener_drop_sequence,
        );
    })?);

    Ok(listeners)
}

fn handle_drop(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    status: &WebElement,
    event: &moirai_pal::wasm::WebEvent,
    generation: Generation,
    drop_task: &Rc<RefCell<Option<LocalTaskHandle>>>,
    drop_sequence: &Rc<Cell<u64>>,
) {
    let files = match event.drop_files() {
        Ok(Some(files)) => files,
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
    let entries = files.files().iter().map(|file| {
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
        Ok(next_state) => {
            update_state(document, state, status, next_state);
            let Some(sequence) = next_sequence(drop_sequence) else {
                update_read_state(document, state, status, DropReadState::Failed);
                return;
            };
            let _ = drop_task.borrow_mut().take();
            update_read_state(document, state, status, DropReadState::Reading);
            read_first_header(
                DropReadContext {
                    document: document.clone(),
                    state: Rc::clone(state),
                    status: status.clone(),
                    generation,
                    task_slot: Rc::clone(drop_task),
                    sequence: Rc::clone(drop_sequence),
                },
                files,
                sequence,
            );
        }
        Err(error) => update_rejection(document, state, status, error, None),
    }
}

fn read_first_header(context: DropReadContext, mut files: DropFiles, sequence: u64) {
    let DropReadContext {
        document,
        state,
        status,
        generation,
        task_slot,
        sequence: task_sequence,
    } = context;
    let task_slot_for_task = Rc::clone(&task_slot);
    let task = spawn_local_with_handle(async move {
        let result = read_header(&mut files).await;
        if !generation_is_current(generation) || task_sequence.get() != sequence {
            // A newer sequence or mount owns the shared slot and its task.
            return;
        }
        let next_state = match result {
            Ok((bytes_read, header)) => DropReadState::Complete { bytes_read, header },
            Err(_) => DropReadState::Failed,
        };
        state.borrow_mut().drop_read_state = next_state;
        if let Err(error) = view::render(&document, &state.borrow()) {
            view::set_status_error(&document, &status, "File bytes", &error.to_string());
        }
        let _ = task_slot_for_task.borrow_mut().take();
    });
    *task_slot.borrow_mut() = Some(task);
}

async fn read_header(files: &mut DropFiles) -> io::Result<(usize, DicomHeader)> {
    let file = files.files_mut().first_mut().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "browser drop contains no readable file",
        )
    })?;
    let mut prefix = [0_u8; 132];
    let bytes_read = file.read(&mut prefix).await?;
    let prefix = prefix.get(..bytes_read).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "browser file reader returned an invalid byte count",
        )
    })?;
    Ok((bytes_read, classify_dicom_header(prefix)))
}

fn next_sequence(sequence: &Cell<u64>) -> Option<u64> {
    let next = sequence.get().checked_add(1)?;
    sequence.set(next);
    Some(next)
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

fn update_read_state(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    status: &WebElement,
    next_state: DropReadState,
) {
    state.borrow_mut().drop_read_state = next_state;
    if let Err(error) = view::render(document, &state.borrow()) {
        view::set_status_error(document, status, "File bytes", &error.to_string());
    }
}

fn update_rejection(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    status: &WebElement,
    error: FileDropError,
    provider_message: Option<String>,
) {
    let mut current = state.borrow_mut();
    current.drop_state = DropState::Rejected(error);
    current.drop_read_state = DropReadState::Failed;
    drop(current);
    if let Err(render_error) = view::render(document, &state.borrow()) {
        view::set_status_error(document, status, "File drop", &render_error.to_string());
        return;
    }
    if let Some(message) = provider_message {
        view::set_status_error(document, status, "File drop", &message);
    }
}
