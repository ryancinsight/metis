//! Browser DOM application boundary.

#[path = "browser/config.rs"]
mod config;
#[path = "browser/dialog.rs"]
mod dialog;
#[path = "browser/events.rs"]
mod events;
#[path = "browser/file_drop.rs"]
mod file_drop;
#[path = "browser/gesture.rs"]
mod gesture;
#[path = "browser/pointer.rs"]
mod pointer;
#[path = "browser/text.rs"]
mod text;
#[path = "view.rs"]
mod view;
#[path = "browser/wheel.rs"]
mod wheel;

use crate::controls;
use crate::controls::{ControlField, DisplayUnit, InputField};
use crate::epoch::{Epoch, Generation};
use crate::session::connect_failure_state;
use metis_core::error::{ErrorCode, MetisError};
use metis_frontend::{AsyncFrontendApp, FormInputs, FormState};
use metis_ipc::BrowserWebSocketTransport;
use metis_ipc::client::HandshakeError;
use moirai_pal::wasm::{
    LocalTaskHandle, WebDocument, WebElement, WebEventListener, spawn_local_with_handle,
};
use std::cell::{Cell, RefCell};
use std::io;
use std::rc::Rc;
use std::time::Duration;

use config::BridgeConfig;

#[derive(Clone)]
struct BrowserState {
    inputs: FormInputs,
    state: FormState,
    bridge: BridgeStatus,
    capabilities: String,
    plugins: String,
    event_status: String,
    drop_state: crate::file_drop_policy::DropState,
    drop_read_state: crate::file_drop_policy::DropReadState,
    text_state: crate::text_policy::TextState,
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
            text_state: crate::text_policy::TextState::default(),
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
    #[expect(
        dead_code,
        reason = "listener handles are retained solely for Drop teardown"
    )]
    listeners: Vec<WebEventListener>,
    state: Rc<RefCell<BrowserState>>,
    app: Rc<RefCell<Option<AsyncFrontendApp<BrowserWebSocketTransport>>>>,
    task: Rc<RefCell<Option<LocalTaskHandle>>>,
    drop_task: Rc<RefCell<Option<LocalTaskHandle>>>,
    generation: Generation,
}

impl BrowserApplication {
    fn mount(document: &WebDocument, generation: Generation) -> io::Result<Self> {
        let bridge_config = config::read_bridge_config(document)?;
        let root = view::element(document, "metis-app")?;
        root.set_inner_html(controls::BROWSER_MARKUP);
        let state = Rc::new(RefCell::new(BrowserState::default()));
        view::render(document, &state.borrow())?;
        let app = Rc::new(RefCell::new(None));
        let task = Rc::new(RefCell::new(None));
        let drop_task = Rc::new(RefCell::new(None));
        let drop_sequence = Rc::new(Cell::new(0));

        let mut listeners = control_listeners(
            document,
            &state,
            &app,
            generation,
            &drop_task,
            &drop_sequence,
        )?;

        let form = view::element(document, "metis-form")?;
        let listener_document = document.clone();
        let listener_state = Rc::clone(&state);
        let listener_app = Rc::clone(&app);
        let listener_task = Rc::clone(&task);
        listeners.push(form.add_event_listener("submit", move |event| {
            event.prevent_default();
            submit(
                &listener_document,
                &listener_state,
                &listener_app,
                &listener_task,
                generation,
            );
        })?);
        let application = Self {
            listeners,
            state,
            app,
            task,
            drop_task,
            generation,
        };
        if let Some(config) = bridge_config {
            application.connect(document, config);
        }
        Ok(application)
    }

    fn connect(&self, document: &WebDocument, config: BridgeConfig) {
        let generation = self.generation;
        let state = Rc::clone(&self.state);
        let app_slot = Rc::clone(&self.app);
        let task_cleanup = Rc::clone(&self.task);
        let listener_document = document.clone();
        state.borrow_mut().bridge = BridgeStatus::Connecting;
        if let Err(error) = view::render(document, &state.borrow()) {
            view::set_mount_error(document, &error);
        }
        let task = spawn_local_with_handle(async move {
            let result = async {
                let transport = BrowserWebSocketTransport::connect_with_defaults_async(
                    &config.endpoint,
                    Duration::from_secs(5),
                )
                .await
                .map_err(HandshakeError::from)?;
                let mut frontend = AsyncFrontendApp::new(transport, Duration::from_secs(5))
                    .map_err(HandshakeError::from)?;
                let inputs = state.borrow().inputs.clone();
                frontend.set_inputs(
                    &inputs.patient_id,
                    inputs.weight_kg,
                    inputs.concentration_mg_ml,
                    inputs.target_dose_mcg_kg_min,
                );
                frontend.init(config.process_id, config.principal).await?;
                Ok::<_, HandshakeError>(frontend)
            }
            .await;
            if !generation_is_current(generation) {
                let _ = task_cleanup.borrow_mut().take();
                return;
            }
            match result {
                Ok(frontend) => {
                    let capabilities =
                        match (frontend.capabilities(), frontend.target_capabilities()) {
                            (Some(catalog), Some(target)) => {
                                view::capability_summary(catalog, target)
                            }
                            _ => "Host capabilities: unavailable".to_owned(),
                        };
                    *app_slot.borrow_mut() = Some(frontend);
                    let mut state = state.borrow_mut();
                    state.bridge = BridgeStatus::Ready;
                    state.state = FormState::Idle;
                    state.capabilities = capabilities;
                }
                Err(error) => {
                    let mut state = state.borrow_mut();
                    state.bridge = BridgeStatus::Disabled;
                    state.state = connect_failure_state(error);
                    "Host capabilities: unavailable".clone_into(&mut state.capabilities);
                }
            }
            if !generation_is_current(generation) {
                let _ = task_cleanup.borrow_mut().take();
                return;
            }
            if let Err(error) = view::render(&listener_document, &state.borrow()) {
                view::set_mount_error(&listener_document, &error);
            }
            let _ = task_cleanup.borrow_mut().take();
        });
        *self.task.borrow_mut() = Some(task);
    }
}

impl Drop for BrowserApplication {
    fn drop(&mut self) {
        let _ = self.task.borrow_mut().take();
        let _ = self.drop_task.borrow_mut().take();
    }
}

fn control_listeners(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    app: &Rc<RefCell<Option<AsyncFrontendApp<BrowserWebSocketTransport>>>>,
    generation: Generation,
    drop_task: &Rc<RefCell<Option<LocalTaskHandle>>>,
    drop_sequence: &Rc<Cell<u64>>,
) -> io::Result<Vec<WebEventListener>> {
    let mut listeners = Vec::with_capacity(34);
    for (id, field) in [
        ("patient-id", InputField::Patient),
        ("weight-kg", InputField::Weight),
        ("concentration-mg-ml", InputField::Concentration),
        ("target-dose", InputField::Dose),
    ] {
        listeners.push(input_listener(document, state, app, id, field)?);
    }
    for (id, event_name, field) in [
        ("show-events", "change", ControlField::ShowEvents),
        (
            "dose-volume",
            "change",
            ControlField::DisplayUnit(DisplayUnit::Volume),
        ),
        (
            "dose-mass",
            "change",
            ControlField::DisplayUnit(DisplayUnit::DrugMass),
        ),
        ("result-scale", "input", ControlField::Scale),
        ("result-detail-select", "change", ControlField::ResultDetail),
    ] {
        listeners.push(control_listener(document, state, id, event_name, field)?);
    }
    listeners.extend(dialog::listeners(document)?);
    listeners.extend(pointer::listeners(document)?);
    listeners.extend(wheel::listeners(document)?);
    listeners.extend(gesture::listeners(document)?);
    listeners.extend(file_drop::listeners(
        document,
        state,
        generation,
        drop_task,
        drop_sequence,
    )?);
    listeners.extend(text::listeners(document, state)?);
    Ok(listeners)
}

fn input_listener(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    app: &Rc<RefCell<Option<AsyncFrontendApp<BrowserWebSocketTransport>>>>,
    id: &'static str,
    field: InputField,
) -> io::Result<WebEventListener> {
    let input = view::element(document, id)?;
    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    let listener_app = Rc::clone(app);
    input.add_event_listener("input", move |event| {
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
    })
}

fn control_listener(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    id: &'static str,
    event_name: &'static str,
    field: ControlField,
) -> io::Result<WebEventListener> {
    let input = view::element(document, id)?;
    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    input.add_event_listener(event_name, move |event| {
        let target = event.target();
        let checked = target.as_ref().and_then(WebElement::checked);
        let value = event.value();
        let mut state = listener_state.borrow_mut();
        let BrowserState {
            controls,
            state: form_state,
            ..
        } = &mut *state;
        controls::update_control(controls, form_state, field, checked, value.as_deref());
        if let Err(error) = view::render(&listener_document, &state) {
            view::set_mount_error(&listener_document, &error);
        }
    })
}

fn submit(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    app_slot: &Rc<RefCell<Option<AsyncFrontendApp<BrowserWebSocketTransport>>>>,
    task_slot: &Rc<RefCell<Option<LocalTaskHandle>>>,
    generation: Generation,
) {
    if !generation_is_current(generation) {
        return;
    }
    if !matches!(state.borrow().bridge, BridgeStatus::Ready) {
        return;
    }
    if task_slot.borrow().is_some() {
        let mut state = state.borrow_mut();
        state.state = FormState::Pending;
        if let Err(error) = view::render(document, &state) {
            view::set_mount_error(document, &error);
        }
        return;
    }
    let Some(mut app) = app_slot.borrow_mut().take() else {
        let mut state = state.borrow_mut();
        state.state = FormState::Disconnected(MetisError::transport(
            ErrorCode::ConnectionClosed,
            "No authorized browser backend bridge is configured",
        ));
        if let Err(error) = view::render(document, &state) {
            view::set_mount_error(document, &error);
        }
        return;
    };
    let inputs = state.borrow().inputs.clone();
    app.set_inputs(
        &inputs.patient_id,
        inputs.weight_kg,
        inputs.concentration_mg_ml,
        inputs.target_dose_mcg_kg_min,
    );
    {
        let mut state = state.borrow_mut();
        state.state = FormState::Pending;
        if let Err(error) = view::render(document, &state) {
            view::set_mount_error(document, &error);
        }
    }
    let task_cleanup = Rc::clone(task_slot);
    let result_state = Rc::clone(state);
    let result_app = Rc::clone(app_slot);
    let result_document = document.clone();
    let task = spawn_local_with_handle(async move {
        let result = app.submit_calculation().await;
        let expected = match app.state() {
            FormState::Success(response) => Some(response.clone()),
            _ => None,
        };
        let event = match expected {
            Some(expected) if result.is_ok() => {
                events::receive_result_event(&mut app, expected).await
            }
            _ => Ok(None),
        };
        let event_error = event.as_ref().err().cloned();
        let outcome = event_error
            .clone()
            .map_or_else(|| app.state().clone(), FormState::Disconnected);
        if !generation_is_current(generation) {
            let _ = task_cleanup.borrow_mut().take();
            return;
        }
        let bridge = if result.is_ok() && event_error.is_none() {
            BridgeStatus::Ready
        } else {
            BridgeStatus::Disabled
        };
        *result_app.borrow_mut() = Some(app);
        {
            let mut state = result_state.borrow_mut();
            state.bridge = bridge;
            if result.is_err() || event_error.is_some() {
                "Host capabilities: unavailable".clone_into(&mut state.capabilities);
            }
            match (&event, &outcome) {
                (Ok(Some(summary)), _) => summary.clone_into(&mut state.event_status),
                (Ok(None), FormState::Rejected(_)) => {
                    "Remote events: none (request rejected)".clone_into(&mut state.event_status);
                }
                (Err(error), _) => {
                    state.event_status =
                        format!("Remote event unavailable [{}]", error.code.as_str());
                }
                _ => {}
            }
            state.state = outcome;
            if let Err(error) = view::render(&result_document, &state) {
                view::set_mount_error(&result_document, &error);
            }
        }
        let _ = task_cleanup.borrow_mut().take();
    });
    *task_slot.borrow_mut() = Some(task);
}

thread_local! {
    static APPLICATION: RefCell<Option<BrowserApplication>> = const { RefCell::new(None) };
    static APPLICATION_EPOCH: RefCell<Epoch> = const { RefCell::new(Epoch::new()) };
}

fn next_generation() -> io::Result<Generation> {
    APPLICATION_EPOCH.with_borrow_mut(Epoch::advance)
}

fn generation_is_current(generation: Generation) -> bool {
    APPLICATION_EPOCH.with_borrow(|epoch| epoch.accepts(generation))
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
    if let Err(error) = next_generation() {
        APPLICATION.with_borrow_mut(|slot| *slot = None);
        if let Ok(document) = WebDocument::current() {
            view::set_mount_error(&document, &error);
        }
        return;
    }
    APPLICATION.with_borrow_mut(|slot| *slot = None);
    if let Ok(document) = WebDocument::current()
        && let Ok(root) = view::element(&document, "metis-app")
    {
        root.set_inner_html("<p>Metis browser host stopped.</p>");
    }
}
