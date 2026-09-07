//! Browser DOM application boundary.

use metis_core::CapabilityScope;
use metis_core::error::{ErrorCode, MetisError};
use metis_core::protocol::{
    CapabilityCatalogPayload, ClinicalCalcResponsePayload, MAX_PLUGINS, Plugin, PluginDescriptor,
    PluginOperation, PluginRegistry,
};
use metis_frontend::{AsyncFrontendApp, FormInputs, FormState};
use metis_ipc::BrowserWebSocketTransport;
use moirai_pal::wasm::{
    LocalTaskHandle, WebDocument, WebElement, WebEventListener, spawn_local_with_handle,
};
use std::cell::RefCell;
use std::io;
use std::rc::Rc;
use std::time::Duration;

const BROWSER_MARKUP: &str = r#"
<header class="metis-header">
  <p class="metis-kicker">METIS / BROWSER WORKBENCH</p>
  <h1>Authorized clinical form boundary</h1>
  <p id="metis-status" role="status">Browser controls are active.</p>
  <p id="metis-capabilities">Host capabilities: unavailable</p>
  <p id="metis-plugins">Registered frontend extensions: unavailable</p>
  <p id="metis-events" role="status">Remote events: none</p>
</header>
<form id="metis-form" class="metis-form">
  <label for="patient-id">Patient reference</label>
  <input id="patient-id" name="patient-id" value="PT-9042-ALPHA" autocomplete="off">
  <label for="weight-kg">Weight (kg)</label>
  <input id="weight-kg" name="weight-kg" type="number" step="any" value="72.5">
  <label for="concentration-mg-ml">Drug concentration (mg/mL)</label>
  <input id="concentration-mg-ml" name="concentration-mg-ml" type="number" step="any" value="4">
  <label for="target-dose">Target dose (mcg/kg/min)</label>
  <input id="target-dose" name="target-dose" type="number" step="any" value="0.5">
  <button id="submit-calculation" type="submit">Submit to authorized backend</button>
</form>
<section class="metis-result" aria-labelledby="result-heading">
  <h2 id="result-heading">Backend result</h2>
  <p id="result-state">No backend bridge configured.</p>
  <dl>
    <dt>Patient</dt><dd id="result-patient">PT-9042-ALPHA</dd>
    <dt>Weight</dt><dd id="result-weight">72.50 kg</dd>
    <dt>Concentration</dt><dd id="result-concentration">4.00 mg/mL</dd>
    <dt>Dose</dt><dd id="result-dose">0.500 mcg/kg/min</dd>
  </dl>
</section>
"#;

#[derive(Clone)]
struct BrowserState {
    inputs: FormInputs,
    state: FormState,
    bridge: BridgeStatus,
    capabilities: String,
    plugins: String,
    event_status: String,
}

impl Default for BrowserState {
    fn default() -> Self {
        Self {
            inputs: FormInputs::new("PT-9042-ALPHA", 72.5, 4.0, 0.5),
            state: FormState::Idle,
            bridge: BridgeStatus::Disabled,
            capabilities: "Host capabilities: unavailable".to_owned(),
            plugins: plugin_summary(),
            event_status: "Remote events: none".to_owned(),
        }
    }
}

#[derive(Clone, Copy)]
enum BridgeStatus {
    Disabled,
    Connecting,
    Ready,
}

struct BridgeConfig {
    endpoint: String,
    process_id: u32,
    principal: [u8; 16],
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
}

impl BrowserApplication {
    fn mount(document: &WebDocument) -> io::Result<Self> {
        let bridge_config = read_bridge_config(document)?;
        let root = element(document, "metis-app")?;
        root.set_inner_html(BROWSER_MARKUP);
        let state = Rc::new(RefCell::new(BrowserState::default()));
        render(document, &state.borrow())?;
        let app = Rc::new(RefCell::new(None));
        let task = Rc::new(RefCell::new(None));

        let mut listeners = Vec::with_capacity(5);
        listeners.push(input_listener(
            document,
            &state,
            &app,
            "patient-id",
            InputField::Patient,
        )?);
        listeners.push(input_listener(
            document,
            &state,
            &app,
            "weight-kg",
            InputField::Weight,
        )?);
        listeners.push(input_listener(
            document,
            &state,
            &app,
            "concentration-mg-ml",
            InputField::Concentration,
        )?);
        listeners.push(input_listener(
            document,
            &state,
            &app,
            "target-dose",
            InputField::Dose,
        )?);

        let form = element(document, "metis-form")?;
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
            );
        })?);
        let application = Self {
            listeners,
            state,
            app,
            task,
        };
        if let Some(config) = bridge_config {
            application.connect(document, config);
        }
        Ok(application)
    }

    fn connect(&self, document: &WebDocument, config: BridgeConfig) {
        let state = Rc::clone(&self.state);
        let app_slot = Rc::clone(&self.app);
        let task_cleanup = Rc::clone(&self.task);
        let listener_document = document.clone();
        state.borrow_mut().bridge = BridgeStatus::Connecting;
        if let Err(error) = render(document, &state.borrow()) {
            set_mount_error(document, &error);
        }
        let task = spawn_local_with_handle(async move {
            let result = async {
                let transport = BrowserWebSocketTransport::connect_with_defaults_async(
                    &config.endpoint,
                    Duration::from_secs(5),
                )
                .await?;
                let mut frontend = AsyncFrontendApp::new(transport, Duration::from_secs(5))?;
                let inputs = state.borrow().inputs.clone();
                frontend.set_inputs(
                    &inputs.patient_id,
                    inputs.weight_kg,
                    inputs.concentration_mg_ml,
                    inputs.target_dose_mcg_kg_min,
                );
                frontend
                    .init(config.process_id, config.principal)
                    .await
                    .map_err(metis_handshake_error)?;
                Ok::<_, MetisError>(frontend)
            }
            .await;
            match result {
                Ok(frontend) => {
                    let capabilities = frontend.capabilities().map_or_else(
                        || "Host capabilities: unavailable".to_owned(),
                        capability_summary,
                    );
                    *app_slot.borrow_mut() = Some(frontend);
                    let mut state = state.borrow_mut();
                    state.bridge = BridgeStatus::Ready;
                    state.state = FormState::Idle;
                    state.capabilities = capabilities;
                }
                Err(error) => {
                    let mut state = state.borrow_mut();
                    state.bridge = BridgeStatus::Disabled;
                    state.state = FormState::Disconnected(error);
                    "Host capabilities: unavailable".clone_into(&mut state.capabilities);
                }
            }
            if let Err(error) = render(&listener_document, &state.borrow()) {
                set_mount_error(&listener_document, &error);
            }
            let _ = task_cleanup.borrow_mut().take();
        });
        *self.task.borrow_mut() = Some(task);
    }
}

#[derive(Clone, Copy)]
enum InputField {
    Patient,
    Weight,
    Concentration,
    Dose,
}

fn input_listener(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    app: &Rc<RefCell<Option<AsyncFrontendApp<BrowserWebSocketTransport>>>>,
    id: &'static str,
    field: InputField,
) -> io::Result<WebEventListener> {
    let input = element(document, id)?;
    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    let listener_app = Rc::clone(app);
    input.add_event_listener("input", move |event| {
        let Some(value) = event.value() else {
            return;
        };
        let mut state = listener_state.borrow_mut();
        update_input(&mut state, field, &value);
        if let Some(app) = listener_app.borrow_mut().as_mut() {
            let inputs = &state.inputs;
            app.set_inputs(
                &inputs.patient_id,
                inputs.weight_kg,
                inputs.concentration_mg_ml,
                inputs.target_dose_mcg_kg_min,
            );
        }
        if let Err(error) = render(&listener_document, &state) {
            set_mount_error(&listener_document, &error);
        }
    })
}

fn update_input(state: &mut BrowserState, field: InputField, value: &str) {
    match field {
        InputField::Patient => value.clone_into(&mut state.inputs.patient_id),
        InputField::Weight => {
            if let Some(value) = parse_finite(value) {
                state.inputs.weight_kg = value;
            } else {
                state.state = invalid_input("weight");
                return;
            }
        }
        InputField::Concentration => {
            if let Some(value) = parse_finite(value) {
                state.inputs.concentration_mg_ml = value;
            } else {
                state.state = invalid_input("concentration");
                return;
            }
        }
        InputField::Dose => {
            if let Some(value) = parse_finite(value) {
                state.inputs.target_dose_mcg_kg_min = value;
            } else {
                state.state = invalid_input("dose");
                return;
            }
        }
    }
    state.state = FormState::Idle;
}

fn submit(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    app_slot: &Rc<RefCell<Option<AsyncFrontendApp<BrowserWebSocketTransport>>>>,
    task_slot: &Rc<RefCell<Option<LocalTaskHandle>>>,
) {
    if task_slot.borrow().is_some() {
        let mut state = state.borrow_mut();
        state.state = FormState::Pending;
        if let Err(error) = render(document, &state) {
            set_mount_error(document, &error);
        }
        return;
    }
    let Some(mut app) = app_slot.borrow_mut().take() else {
        let mut state = state.borrow_mut();
        state.state = FormState::Disconnected(MetisError::transport(
            ErrorCode::ConnectionClosed,
            "No authorized browser backend bridge is configured",
        ));
        if let Err(error) = render(document, &state) {
            set_mount_error(document, &error);
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
        if let Err(error) = render(document, &state) {
            set_mount_error(document, &error);
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
            Some(expected) if result.is_ok() => match app.recv_event().await {
                Ok(event) => match event.decode_as::<ClinicalCalcResponsePayload>() {
                    Ok(received) if received == expected => Ok(Some(format!(
                        "Remote event: {} #{} (audit={} rate={:.6} ml/hr drug={:.6} mg/hr)",
                        event.name(),
                        event.event_id().get(),
                        received.audit_sequence_id,
                        received.rate_ml_hr,
                        received.drug_rate_mg_hr,
                    ))),
                    Ok(_) => Err(MetisError::protocol(
                        ErrorCode::SequenceMismatch,
                        "Remote event result differs from its correlated response",
                    )),
                    Err(error) => Err(error),
                },
                Err(error) => Err(error),
            },
            _ => Ok(None),
        };
        let event_error = event.as_ref().err().cloned();
        let outcome = event_error
            .clone()
            .map_or_else(|| app.state().clone(), FormState::Disconnected);
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
            if let Err(error) = render(&result_document, &state) {
                set_mount_error(&result_document, &error);
            }
        }
        let _ = task_cleanup.borrow_mut().take();
    });
    *task_slot.borrow_mut() = Some(task);
}

fn metis_handshake_error(error: metis_ipc::client::HandshakeError) -> MetisError {
    match error {
        metis_ipc::client::HandshakeError::Local(error) => error,
        metis_ipc::client::HandshakeError::Remote(response) => MetisError::capability(
            ErrorCode::InvalidPrincipal,
            format!("Backend rejected browser session: {}", response.message),
        ),
        _ => MetisError::transport(
            ErrorCode::TransportBroken,
            "Browser handshake failed with an unknown result",
        ),
    }
}

fn read_bridge_config(document: &WebDocument) -> io::Result<Option<BridgeConfig>> {
    let Some(endpoint) = optional_value(document, "metis-websocket-endpoint") else {
        return Ok(None);
    };
    if endpoint.trim().is_empty() {
        return Ok(None);
    }
    if !endpoint.starts_with("ws://") && !endpoint.starts_with("wss://") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Browser endpoint must use ws:// or wss://",
        ));
    }
    let process_id = optional_value(document, "metis-process-id")
        .ok_or_else(|| config_error("Browser process identifier is missing"))?
        .parse::<u32>()
        .map_err(|_| config_error("Browser process identifier is not a positive integer"))?;
    if process_id == 0 {
        return Err(config_error("Browser process identifier must be nonzero"));
    }
    let principal = optional_value(document, "metis-principal")
        .ok_or_else(|| config_error("Browser principal is missing"))?;
    Ok(Some(BridgeConfig {
        endpoint,
        process_id,
        principal: parse_principal(&principal)?,
    }))
}

fn optional_value(document: &WebDocument, id: &str) -> Option<String> {
    document
        .get_element_by_id(id)
        .and_then(|element| element.value())
}

fn parse_principal(value: &str) -> io::Result<[u8; 16]> {
    let bytes = value.as_bytes();
    if bytes.len() != 32 {
        return Err(config_error("Browser principal must contain 32 hex digits"));
    }
    let mut principal = [0; 16];
    for (index, slot) in principal.iter_mut().enumerate() {
        let high = hex_digit(bytes[index * 2])?;
        let low = hex_digit(bytes[index * 2 + 1])?;
        *slot = (high << 4) | low;
    }
    if principal == [0; 16] {
        return Err(config_error("Browser principal must be nonzero"));
    }
    Ok(principal)
}

fn hex_digit(value: u8) -> io::Result<u8> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(config_error("Browser principal contains a non-hex digit")),
    }
}

fn config_error(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn parse_finite(value: &str) -> Option<f64> {
    value.parse::<f64>().ok().filter(|value| value.is_finite())
}

fn invalid_input(field: &str) -> FormState {
    FormState::Failed(MetisError::clinical(
        ErrorCode::NumericInstability,
        format!("Browser field {field} must contain a finite number"),
    ))
}

fn render(document: &WebDocument, state: &BrowserState) -> io::Result<()> {
    let inputs = &state.inputs;
    set_text(document, "result-patient", &inputs.patient_id)?;
    set_text(
        document,
        "result-weight",
        &format!("{:.2} kg", inputs.weight_kg),
    )?;
    set_text(
        document,
        "result-concentration",
        &format!("{:.2} mg/mL", inputs.concentration_mg_ml),
    )?;
    set_text(
        document,
        "result-dose",
        &format!("{:.3} mcg/kg/min", inputs.target_dose_mcg_kg_min),
    )?;
    let message = match &state.state {
        FormState::Idle => match state.bridge {
            BridgeStatus::Disabled => "Controls active; no authorized backend bridge configured",
            BridgeStatus::Connecting => "Connecting to authorized backend",
            BridgeStatus::Ready => "Authorized backend session ready",
        }
        .to_owned(),
        FormState::Failed(error) => format!("Input rejected [{}]", error.code.as_str()),
        FormState::Disconnected(error) => {
            format!("Backend unavailable [{}]", error.code.as_str())
        }
        FormState::Pending => "Request in progress".to_owned(),
        FormState::Success(_) => "Backend result received".to_owned(),
        FormState::Rejected(error) => {
            format!("Backend rejected request [0x{:04X}]", error.error_code)
        }
        FormState::SessionFailed(_) => "Backend session failed".to_owned(),
        _ => "Unsupported form state".to_owned(),
    };
    set_text(document, "metis-status", &message)?;
    set_text(document, "metis-capabilities", &state.capabilities)?;
    set_text(document, "metis-plugins", &state.plugins)?;
    set_text(document, "metis-events", &state.event_status)?;
    set_text(document, "result-state", &message)
}

fn capability_summary(catalog: &CapabilityCatalogPayload) -> String {
    let names = catalog
        .commands()
        .iter()
        .filter_map(|command| {
            command
                .descriptor()
                .map(metis_core::CommandDescriptor::name)
        })
        .collect::<Vec<_>>();
    if names.is_empty() {
        return "Host capabilities: none advertised".to_owned();
    }
    format!("Host capabilities: {}", names.join(", "))
}

static WORKBENCH_EVENTS: [PluginOperation; 1] = [PluginOperation::new(
    "form.state",
    CapabilityScope::UI_RENDER,
)];

struct WorkbenchPlugin;

impl Plugin for WorkbenchPlugin {
    const DESCRIPTOR: PluginDescriptor =
        PluginDescriptor::new("workbench", 1, &[], &WORKBENCH_EVENTS);
}

fn plugin_summary() -> String {
    let mut registry = PluginRegistry::<MAX_PLUGINS>::new()
        .expect("invariant: the browser plugin registry has a positive bounded capacity");
    registry
        .register::<WorkbenchPlugin>()
        .expect("invariant: the static browser plugin manifest is valid");
    let entries = registry
        .plugins()
        .iter()
        .map(|plugin| format!("{} v{}", plugin.name(), plugin.version()))
        .collect::<Vec<_>>();
    format!("Registered frontend extensions: {}", entries.join(", "))
}

fn set_text(document: &WebDocument, id: &str, text: &str) -> io::Result<()> {
    element(document, id)?.set_text(text);
    Ok(())
}

fn element(document: &WebDocument, id: &str) -> io::Result<WebElement> {
    document.get_element_by_id(id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("Metis DOM element #{id} is absent"),
        )
    })
}

fn set_mount_error(document: &WebDocument, error: &io::Error) {
    if let Ok(status) = element(document, "metis-status") {
        status.set_text(&format!("Browser host error: {error}"));
    }
}

thread_local! {
    static APPLICATION: RefCell<Option<BrowserApplication>> = const { RefCell::new(None) };
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
    APPLICATION.with_borrow_mut(|slot| *slot = None);
    let result = WebDocument::current().and_then(|document| BrowserApplication::mount(&document));
    match result {
        Ok(application) => APPLICATION.with_borrow_mut(|slot| *slot = Some(application)),
        Err(error) => {
            if let Ok(document) = WebDocument::current() {
                set_mount_error(&document, &error);
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
    APPLICATION.with_borrow_mut(|slot| *slot = None);
    if let Ok(document) = WebDocument::current()
        && let Ok(root) = element(&document, "metis-app")
    {
        root.set_inner_html("<p>Metis browser host stopped.</p>");
    }
}
