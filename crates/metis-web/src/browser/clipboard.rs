//! Browser clipboard controls backed by Moirai's bounded provider.

use super::{BrowserState, generation_is_current, view};
use moirai_pal::wasm::{
    LocalTaskHandle, WebClipboard, WebDocument, WebElement, WebEventListener,
    spawn_local_with_handle,
};
use std::cell::RefCell;
use std::io;
use std::rc::Rc;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum ClipboardStatus {
    #[default]
    Ready,
    Unavailable,
    Reading,
    Writing,
    Read {
        bytes: usize,
    },
    Written {
        bytes: usize,
    },
    Failed {
        operation: &'static str,
        message: String,
    },
}

impl ClipboardStatus {
    pub(crate) const fn state_name(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Unavailable => "unavailable",
            Self::Reading => "reading",
            Self::Writing => "writing",
            Self::Read { .. } => "read",
            Self::Written { .. } => "written",
            Self::Failed { .. } => "failed",
        }
    }

    pub(crate) fn message(&self) -> String {
        match self {
            Self::Ready => "Clipboard: ready; choose an action".to_owned(),
            Self::Unavailable => "Clipboard: unavailable in this browser context".to_owned(),
            Self::Reading => "Clipboard: reading text".to_owned(),
            Self::Writing => "Clipboard: writing text".to_owned(),
            Self::Read { bytes } => format!("Clipboard: read {bytes} UTF-8 bytes into the note"),
            Self::Written { bytes } => format!("Clipboard: wrote {bytes} UTF-8 bytes"),
            Self::Failed { operation, message } => {
                format!("Clipboard: {operation} failed ({message})")
            }
        }
    }
}

pub(super) fn listeners(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    generation: crate::epoch::Generation,
    task_slot: &Rc<RefCell<Option<LocalTaskHandle>>>,
) -> io::Result<Vec<WebEventListener>> {
    let provider = if let Ok(provider) = document.clipboard() {
        Some(provider)
    } else {
        state.borrow_mut().clipboard_status = ClipboardStatus::Unavailable;
        None
    };
    let read_button = view::element(document, "clipboard-read")?;
    let write_button = view::element(document, "clipboard-write")?;
    if provider.is_none() {
        view::render(document, &state.borrow())?;
    }

    Ok(vec![
        read_button_listener(
            document,
            state,
            generation,
            task_slot,
            provider.clone(),
            &read_button,
        )?,
        write_button_listener(
            document,
            state,
            generation,
            task_slot,
            provider,
            &write_button,
        )?,
    ])
}

fn read_button_listener(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    generation: crate::epoch::Generation,
    task_slot: &Rc<RefCell<Option<LocalTaskHandle>>>,
    provider: Option<WebClipboard>,
    button: &WebElement,
) -> io::Result<WebEventListener> {
    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    let listener_task = Rc::clone(task_slot);
    button.add_event_listener("click", move |_event| {
        let Some(provider) = provider.clone() else {
            set_status(
                &listener_document,
                &listener_state,
                ClipboardStatus::Unavailable,
            );
            return;
        };
        if listener_task.borrow().is_some() {
            return;
        }
        let read = provider.read_text();
        set_status(
            &listener_document,
            &listener_state,
            ClipboardStatus::Reading,
        );
        let task_cleanup = Rc::clone(&listener_task);
        let task_state = Rc::clone(&listener_state);
        let task_document = listener_document.clone();
        let task = spawn_local_with_handle(async move {
            let result = read.await;
            if !generation_is_current(generation) {
                let _ = task_cleanup.borrow_mut().take();
                return;
            }
            match result {
                Ok(text) => {
                    let bytes = text.len();
                    let status = {
                        let mut state = task_state.borrow_mut();
                        match view::element(&task_document, "text-specimen") {
                            Ok(control) => match control.set_value(&text) {
                                Ok(()) => match state.text_state.apply_clipboard(text) {
                                    Ok(()) => ClipboardStatus::Read { bytes },
                                    Err(error) => ClipboardStatus::Failed {
                                        operation: "read",
                                        message: error.to_string(),
                                    },
                                },
                                Err(error) => ClipboardStatus::Failed {
                                    operation: "read",
                                    message: error.to_string(),
                                },
                            },
                            Err(error) => ClipboardStatus::Failed {
                                operation: "read",
                                message: error.to_string(),
                            },
                        }
                    };
                    task_state.borrow_mut().clipboard_status = status;
                }
                Err(error) => {
                    task_state.borrow_mut().clipboard_status = ClipboardStatus::Failed {
                        operation: "read",
                        message: error.to_string(),
                    };
                }
            }
            if let Err(error) = view::render(&task_document, &task_state.borrow()) {
                view::set_mount_error(&task_document, &error);
            }
            let _ = task_cleanup.borrow_mut().take();
        });
        *listener_task.borrow_mut() = Some(task);
    })
}

fn write_button_listener(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    generation: crate::epoch::Generation,
    task_slot: &Rc<RefCell<Option<LocalTaskHandle>>>,
    provider: Option<WebClipboard>,
    button: &WebElement,
) -> io::Result<WebEventListener> {
    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    let listener_task = Rc::clone(task_slot);
    let control = view::element(document, "text-specimen")?;
    button.add_event_listener("click", move |_event| {
        let Some(provider) = provider.clone() else {
            set_status(
                &listener_document,
                &listener_state,
                ClipboardStatus::Unavailable,
            );
            return;
        };
        if listener_task.borrow().is_some() {
            return;
        }
        let value = match control.text_value() {
            Ok(Some(value)) => value,
            Ok(None) => {
                set_status(
                    &listener_document,
                    &listener_state,
                    ClipboardStatus::Failed {
                        operation: "write",
                        message: "the note is not a text control".to_owned(),
                    },
                );
                return;
            }
            Err(error) => {
                set_status(
                    &listener_document,
                    &listener_state,
                    ClipboardStatus::Failed {
                        operation: "write",
                        message: error.to_string(),
                    },
                );
                return;
            }
        };
        let bytes = value.len();
        let write = match provider.write_text(&value) {
            Ok(write) => write,
            Err(error) => {
                set_status(
                    &listener_document,
                    &listener_state,
                    ClipboardStatus::Failed {
                        operation: "write",
                        message: error.to_string(),
                    },
                );
                return;
            }
        };
        set_status(
            &listener_document,
            &listener_state,
            ClipboardStatus::Writing,
        );
        let task_cleanup = Rc::clone(&listener_task);
        let task_state = Rc::clone(&listener_state);
        let task_document = listener_document.clone();
        let task = spawn_local_with_handle(async move {
            let status = match write.await {
                Ok(()) => ClipboardStatus::Written { bytes },
                Err(error) => ClipboardStatus::Failed {
                    operation: "write",
                    message: error.to_string(),
                },
            };
            if generation_is_current(generation) {
                task_state.borrow_mut().clipboard_status = status;
                if let Err(error) = view::render(&task_document, &task_state.borrow()) {
                    view::set_mount_error(&task_document, &error);
                }
            }
            let _ = task_cleanup.borrow_mut().take();
        });
        *listener_task.borrow_mut() = Some(task);
    })
}

fn set_status(document: &WebDocument, state: &Rc<RefCell<BrowserState>>, status: ClipboardStatus) {
    state.borrow_mut().clipboard_status = status;
    if let Err(error) = view::render(document, &state.borrow()) {
        view::set_mount_error(document, &error);
    }
}
