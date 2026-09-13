//! Browser DOM application boundary.

#[path = "browser/application.rs"]
mod application;
#[path = "browser/config.rs"]
mod config;
#[path = "browser/dialog.rs"]
mod dialog;
#[path = "browser/events.rs"]
mod events;
#[path = "browser/explorer.rs"]
mod explorer;
#[path = "browser/file_drop.rs"]
mod file_drop;
#[path = "browser/gesture.rs"]
mod gesture;
#[path = "browser/listeners.rs"]
mod listeners;
#[path = "browser/pointer.rs"]
mod pointer;
#[path = "browser/result.rs"]
mod result;
#[path = "browser/submission.rs"]
mod submission;
#[path = "browser/text.rs"]
mod text;
#[path = "view.rs"]
mod view;
#[path = "browser/wheel.rs"]
mod wheel;

use crate::FileDropBatch;
use crate::controls;
use crate::epoch::{Epoch, Generation};
use crate::fragment;
use crate::session::connect_failure_state;
use metis_frontend::{AsyncFrontendApp, FormInputs, FormState};
use metis_ipc::BrowserWebSocketTransport;
use moirai_pal::wasm::{LocalTaskHandle, WebDocument, WebEventListener};
use std::cell::RefCell;
use std::io;
use std::rc::Rc;

struct BrowserState {
    inputs: FormInputs,
    state: FormState,
    bridge: BridgeStatus,
    capabilities: String,
    plugins: String,
    event_status: String,
    drop_state: crate::file_drop_policy::DropState,
    drop_read_state: crate::file_drop_policy::DropReadState,
    drop_batch: Option<FileDropBatch>,
    text_state: crate::text_policy::TextState,
    result_explorer: metis_frontend::ResultExplorer,
    controls: controls::ControlState,
}

impl Default for BrowserState {
    fn default() -> Self {
        Self {
            inputs: FormInputs::new("PT-9042-ALPHA", 72.5, 4.0, 0.5),
            state: FormState::Idle,
            bridge: BridgeStatus::Disabled,
            capabilities: "Host capabilities: unavailable".to_owned(),
            plugins: view::plugin_summary(),
            event_status: "Remote events: none".to_owned(),
            drop_state: crate::file_drop_policy::DropState::default(),
            drop_read_state: crate::file_drop_policy::DropReadState::default(),
            drop_batch: None,
            text_state: crate::text_policy::TextState::default(),
            result_explorer: metis_frontend::ResultExplorer::new(),
            controls: controls::ControlState::default(),
        }
    }
}

#[derive(Clone, Copy)]
enum BridgeStatus {
    Disabled,
    Connecting,
    Ready,
}

struct BrowserApplication {
    listeners: Vec<WebEventListener>,
    state: Rc<RefCell<BrowserState>>,
    app: Rc<RefCell<Option<AsyncFrontendApp<BrowserWebSocketTransport>>>>,
    task: Rc<RefCell<Option<LocalTaskHandle>>>,
    drop_task: Rc<RefCell<Option<LocalTaskHandle>>>,
    fragment_task: Rc<RefCell<Option<LocalTaskHandle>>>,
    generation: Generation,
}

thread_local! {
    static APPLICATION: RefCell<Option<BrowserApplication>> = const { RefCell::new(None) };
    static APPLICATION_EPOCH: RefCell<Epoch> = const { RefCell::new(Epoch::new()) };
}

fn next_generation() -> io::Result<Generation> {
    APPLICATION_EPOCH.with_borrow_mut(Epoch::advance)
}

pub(crate) fn generation_is_current(generation: Generation) -> bool {
    APPLICATION_EPOCH.with_borrow(|epoch| epoch.accepts(generation))
}

pub(super) fn take_file_drop() -> Option<FileDropBatch> {
    APPLICATION.with_borrow_mut(|slot| {
        slot.as_ref()
            .and_then(|application| application.state.borrow_mut().drop_batch.take())
    })
}

/// Mounts the Metis browser application into the page's `#metis-app` element.
///
/// The export is intentionally a single no-argument WASM boundary. The page
/// owns CSS and the document shell; all mutable form state and event transitions
/// remain in Rust. An existing application is stopped before a remount so a
/// failed mount cannot leave listeners attached to replaced markup. The unsafe
/// attribute is required only to keep this stable raw WASM export callable by
/// the generated browser loader.
#[expect(
    unsafe_code,
    reason = "stable raw WASM export ABI at the browser boundary"
)]
#[unsafe(no_mangle)]
pub extern "C" fn metis_start() {
    let generation = match next_generation() {
        Ok(generation) => generation,
        Err(error) => {
            APPLICATION.with_borrow_mut(|slot| *slot = None);
            if let Ok(document) = WebDocument::current() {
                view::set_mount_error(&document, &error);
            }
            return;
        }
    };
    APPLICATION.with_borrow_mut(|slot| *slot = None);
    let result = WebDocument::current()
        .and_then(|document| BrowserApplication::mount(&document, generation));
    match result {
        Ok(application) => APPLICATION.with_borrow_mut(|slot| *slot = Some(application)),
        Err(error) => {
            if let Ok(document) = WebDocument::current() {
                if let Err(lifecycle_error) = view::render_lifecycle(
                    &document,
                    0,
                    generation.value(),
                    "Lifecycle: mount failed; no Rust-owned listeners",
                ) {
                    view::set_mount_error(&document, &lifecycle_error);
                }
                view::set_mount_error(&document, &error);
            }
        }
    }
}

/// Stops the browser application and releases every Rust-owned DOM listener.
///
/// A subsequent [`metis_start`] call creates fresh state and listeners. The
/// export is intentionally paired with the start boundary so a host can tear
/// down a page or replace a running application without retaining callbacks.
#[expect(
    unsafe_code,
    reason = "stable raw WASM export ABI at the browser boundary"
)]
#[unsafe(no_mangle)]
pub extern "C" fn metis_stop() {
    let generation = match next_generation() {
        Ok(generation) => generation,
        Err(error) => {
            APPLICATION.with_borrow_mut(|slot| *slot = None);
            if let Ok(document) = WebDocument::current() {
                view::set_mount_error(&document, &error);
            }
            return;
        }
    };
    APPLICATION.with_borrow_mut(|slot| *slot = None);
    if let Ok(document) = WebDocument::current()
        && let Ok(root) = view::element(&document, "metis-app")
    {
        if let Err(error) = root.set_attribute("data-metis-listener-count", "0") {
            view::set_mount_error(&document, &error);
            return;
        }
        if let Err(error) =
            root.set_attribute("data-metis-generation", &generation.value().to_string())
        {
            view::set_mount_error(&document, &error);
            return;
        }
        root.set_inner_html(&format!(
            "<p id=\"metis-lifecycle\" role=\"status\" data-listener-count=\"0\" data-generation=\"{}\">Lifecycle: stopped; Rust-owned listeners released (0 listener handles; generation {})</p><p>Metis browser host stopped.</p>",
            generation.value(),
            generation.value(),
        ));
    }
}
