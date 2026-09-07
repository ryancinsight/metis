//! Browser DOM application boundary.

use metis_core::error::{ErrorCode, MetisError};
use metis_frontend::{FormInputs, FormState};
use moirai_pal::wasm::{WebDocument, WebElement, WebEventListener};
use std::cell::RefCell;
use std::io;
use std::rc::Rc;

const BROWSER_MARKUP: &str = r#"
<header class="metis-header">
  <p class="metis-kicker">METIS / BROWSER WORKBENCH</p>
  <h1>Authorized clinical form boundary</h1>
  <p id="metis-status" role="status">Browser controls are active.</p>
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
}

impl Default for BrowserState {
    fn default() -> Self {
        Self {
            inputs: FormInputs::new("PT-9042-ALPHA", 72.5, 4.0, 0.5),
            state: FormState::Idle,
        }
    }
}

struct BrowserApplication {
    #[expect(
        dead_code,
        reason = "listener handles are retained solely for Drop teardown"
    )]
    listeners: Vec<WebEventListener>,
}

impl BrowserApplication {
    fn mount(document: &WebDocument) -> io::Result<Self> {
        let root = element(document, "metis-app")?;
        root.set_inner_html(BROWSER_MARKUP);
        let state = Rc::new(RefCell::new(BrowserState::default()));
        render(document, &state.borrow())?;

        let mut listeners = Vec::with_capacity(5);
        listeners.push(input_listener(
            document,
            &state,
            "patient-id",
            InputField::Patient,
        )?);
        listeners.push(input_listener(
            document,
            &state,
            "weight-kg",
            InputField::Weight,
        )?);
        listeners.push(input_listener(
            document,
            &state,
            "concentration-mg-ml",
            InputField::Concentration,
        )?);
        listeners.push(input_listener(
            document,
            &state,
            "target-dose",
            InputField::Dose,
        )?);

        let form = element(document, "metis-form")?;
        let listener_document = document.clone();
        let listener_state = Rc::clone(&state);
        listeners.push(form.add_event_listener("submit", move |event| {
            event.prevent_default();
            let mut state = listener_state.borrow_mut();
            state.state = FormState::Disconnected(MetisError::transport(
                ErrorCode::ConnectionClosed,
                "No authorized browser backend bridge is configured",
            ));
            if let Err(error) = render(&listener_document, &state) {
                set_mount_error(&listener_document, &error);
            }
        })?);
        Ok(Self { listeners })
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
    id: &'static str,
    field: InputField,
) -> io::Result<WebEventListener> {
    let input = element(document, id)?;
    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    input.add_event_listener("input", move |event| {
        let Some(value) = event.value() else {
            return;
        };
        let mut state = listener_state.borrow_mut();
        update_input(&mut state, field, &value);
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
        FormState::Idle => "Controls active; no authorized backend bridge configured".to_owned(),
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
    set_text(document, "result-state", &message)
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
/// remain in Rust. The unsafe attribute is required only to keep this stable
/// raw WASM export callable by the generated browser loader.
#[expect(
    unsafe_code,
    reason = "stable raw WASM export ABI at the browser boundary"
)]
#[unsafe(no_mangle)]
pub extern "C" fn metis_start() {
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
