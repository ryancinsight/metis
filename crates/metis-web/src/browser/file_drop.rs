//! Browser file-drop listeners and rendering transitions.

use super::BrowserState;
use super::generation_is_current;
use super::view;
use crate::epoch::Generation;
use crate::file_drop_policy::{
    DropReadState, DropState, FileDropEntry, FileDropError, PayloadSizeError, check_payload_size,
};
use crate::{FileDropBatch, FileDropPayload};
use moirai_pal::wasm::{
    BrowserFiles, DropFiles, DroppedFileAccess, LocalTaskHandle, MAX_READ_BYTES, WebDocument,
    WebElement, WebEvent, WebEventListener, spawn_local_with_handle,
};
use std::cell::{Cell, RefCell};
use std::io;
use std::rc::Rc;

const READ_CHUNK_BYTES: usize = 64 * 1024;

struct DropReadContext {
    document: WebDocument,
    state: Rc<RefCell<BrowserState>>,
    status: WebElement,
    generation: Generation,
    task_slot: Rc<RefCell<Option<LocalTaskHandle>>>,
    sequence: Rc<Cell<u64>>,
}

#[derive(Clone, Copy)]
struct DropInputContext<'a> {
    document: &'a WebDocument,
    state: &'a Rc<RefCell<BrowserState>>,
    zone: &'a WebElement,
    status: &'a WebElement,
    generation: Generation,
    drop_task: &'a Rc<RefCell<Option<LocalTaskHandle>>>,
    drop_sequence: &'a Rc<Cell<u64>>,
}

pub(super) fn listeners(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    generation: Generation,
    drop_task: &Rc<RefCell<Option<LocalTaskHandle>>>,
    drop_sequence: &Rc<Cell<u64>>,
) -> io::Result<Vec<WebEventListener>> {
    let zone = view::element(document, "drop-zone")?;
    let file_input = view::element(document, "file-input")?;
    let status = view::element(document, "drop-byte-status")?;
    let mut listeners = Vec::with_capacity(5);

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
            FileInputSource::Drop,
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
                FileInputSource::Drop,
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
            FileInputSource::Drop,
        );
    })?);

    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    let listener_zone = zone.clone();
    let listener_status = status.clone();
    let listener_drop_task = Rc::clone(drop_task);
    let listener_drop_sequence = Rc::clone(drop_sequence);
    listeners.push(zone.add_event_listener("drop", move |event| {
        event.prevent_default();
        handle_drop(
            DropInputContext {
                document: &listener_document,
                state: &listener_state,
                zone: &listener_zone,
                status: &listener_status,
                generation,
                drop_task: &listener_drop_task,
                drop_sequence: &listener_drop_sequence,
            },
            &event,
        );
    })?);

    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    let listener_zone = zone.clone();
    let listener_status = status.clone();
    let listener_drop_task = Rc::clone(drop_task);
    let listener_drop_sequence = Rc::clone(drop_sequence);
    listeners.push(file_input.add_event_listener("change", move |event| {
        handle_selection(
            DropInputContext {
                document: &listener_document,
                state: &listener_state,
                zone: &listener_zone,
                status: &listener_status,
                generation,
                drop_task: &listener_drop_task,
                drop_sequence: &listener_drop_sequence,
            },
            &event,
        );
    })?);

    Ok(listeners)
}

fn handle_drop(context: DropInputContext<'_>, event: &WebEvent) {
    let DropInputContext {
        document,
        state,
        zone,
        status,
        generation,
        drop_task,
        drop_sequence,
    } = context;
    let files = match event.drop_files() {
        Ok(Some(files)) => files,
        Ok(None) => {
            update_rejection(
                document,
                state,
                status,
                FileDropError::ProviderMetadata,
                None,
                FileInputSource::Drop,
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
                FileInputSource::Drop,
            );
            return;
        }
    };
    handle_files(
        DropInputContext {
            document,
            state,
            zone,
            status,
            generation,
            drop_task,
            drop_sequence,
        },
        files,
        FileInputSource::Drop,
    );
}

fn handle_selection(context: DropInputContext<'_>, event: &WebEvent) {
    let DropInputContext {
        document,
        state,
        zone,
        status,
        generation,
        drop_task,
        drop_sequence,
    } = context;
    let files = match event.selected_files() {
        Ok(Some(files)) => files,
        Ok(None) => {
            update_rejection(
                document,
                state,
                status,
                FileDropError::ProviderMetadata,
                None,
                FileInputSource::Selection,
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
                FileInputSource::Selection,
            );
            return;
        }
    };
    handle_files(
        DropInputContext {
            document,
            state,
            zone,
            status,
            generation,
            drop_task,
            drop_sequence,
        },
        files,
        FileInputSource::Selection,
    );
}

fn handle_files<B: BrowserFileBatch + 'static>(
    context: DropInputContext<'_>,
    files: B,
    source: FileInputSource,
) {
    let DropInputContext {
        document,
        state,
        zone,
        status,
        generation,
        drop_task,
        drop_sequence,
    } = context;
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
            update_state(document, state, zone, next_state, source);
            state.borrow_mut().drop_batch = None;
            let Some(sequence) = next_sequence(drop_sequence) else {
                update_read_state(document, state, status, DropReadState::Failed);
                return;
            };
            let _ = drop_task.borrow_mut().take();
            update_read_state(document, state, status, DropReadState::Reading);
            read_drop_batch(
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
        Err(error) => update_rejection(document, state, status, error, None, source),
    }
}

trait BrowserFileBatch {
    fn files(&self) -> &[DroppedFileAccess];
    fn files_mut(&mut self) -> &mut [DroppedFileAccess];
}

impl BrowserFileBatch for DropFiles {
    fn files(&self) -> &[DroppedFileAccess] {
        self.files()
    }

    fn files_mut(&mut self) -> &mut [DroppedFileAccess] {
        self.files_mut()
    }
}

impl BrowserFileBatch for BrowserFiles {
    fn files(&self) -> &[DroppedFileAccess] {
        self.files()
    }

    fn files_mut(&mut self) -> &mut [DroppedFileAccess] {
        self.files_mut()
    }
}

fn read_drop_batch<B: BrowserFileBatch + 'static>(
    context: DropReadContext,
    mut files: B,
    sequence: u64,
) {
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
        let result = read_batch(&mut files).await;
        if !generation_is_current(generation) || task_sequence.get() != sequence {
            // A newer sequence or mount owns the shared slot and its task.
            return;
        }
        if let Ok(batch) = result {
            let bytes_read = batch.total_bytes();
            let files_read = batch.file_count();
            let mut state = state.borrow_mut();
            state.drop_batch = Some(batch);
            state.drop_read_state = DropReadState::Complete {
                bytes_read,
                files_read,
            };
        } else {
            let mut state = state.borrow_mut();
            state.drop_batch = None;
            state.drop_read_state = DropReadState::Failed;
        }
        if let Err(error) = view::render(&document, &state.borrow()) {
            view::set_status_error(&document, &status, "File bytes", &error.to_string());
        }
        let _ = task_slot_for_task.borrow_mut().take();
    });
    *task_slot.borrow_mut() = Some(task);
}

async fn read_batch<B: BrowserFileBatch>(files: &mut B) -> io::Result<FileDropBatch> {
    let mut payloads = Vec::with_capacity(files.files().len());
    let mut total_bytes = 0_u64;
    for file in files.files_mut() {
        let size_bytes = file.size_bytes();
        let expected = check_payload_size(size_bytes, total_bytes).map_err(payload_error)?;
        let bytes = read_file(file, expected).await?;
        total_bytes = total_bytes
            .checked_add(size_bytes)
            .ok_or_else(|| payload_error(PayloadSizeError::BatchTooLarge))?;
        payloads.push(FileDropPayload::from_parts(
            file.name().to_owned(),
            file.media_type().to_owned(),
            bytes.into_boxed_slice(),
        ));
    }
    Ok(FileDropBatch::from_payloads(payloads))
}

async fn read_file(file: &mut DroppedFileAccess, expected: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected)
        .map_err(|_| io::Error::other("browser file payload allocation failed"))?;
    let chunk_capacity = expected.clamp(READ_CHUNK_BYTES, MAX_READ_BYTES);
    let mut chunk = vec![0_u8; chunk_capacity].into_boxed_slice();
    while bytes.len() < expected {
        let request = (expected - bytes.len()).min(chunk.len());
        let count = file.read(&mut chunk[..request]).await?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "browser file ended before its declared size",
            ));
        }
        if count > request {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "browser file reader returned more bytes than requested",
            ));
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    let expected_position = u64::try_from(expected).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "browser file payload size cannot be represented",
        )
    })?;
    if file.position() != expected_position {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "browser file reader position disagrees with its declared size",
        ));
    }
    Ok(bytes)
}

fn payload_error(error: PayloadSizeError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error.to_string())
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
    source: FileInputSource,
) {
    state.borrow_mut().drop_state = next_state;
    if let Err(error) = view::render(document, &state.borrow()) {
        view::set_status_error(document, zone, source.label(), &error.to_string());
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
    source: FileInputSource,
) {
    let mut current = state.borrow_mut();
    current.drop_state = DropState::Rejected(error);
    current.drop_read_state = DropReadState::Failed;
    current.drop_batch = None;
    drop(current);
    if let Err(render_error) = view::render(document, &state.borrow()) {
        view::set_status_error(document, status, source.label(), &render_error.to_string());
        return;
    }
    if let Some(message) = provider_message {
        view::set_status_error(document, status, source.label(), &message);
    }
}

#[derive(Clone, Copy)]
enum FileInputSource {
    Drop,
    Selection,
}

impl FileInputSource {
    const fn label(self) -> &'static str {
        match self {
            Self::Drop => "File drop",
            Self::Selection => "File selection",
        }
    }
}
