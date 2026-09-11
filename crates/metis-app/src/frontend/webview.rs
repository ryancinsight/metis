//! Windows `WebView2` host for the supervised form workflow.

use metis_core::error::{ErrorCode, MetisError, Result};
use metis_frontend::{FormState, FrontendApp};
use metis_ipc::{IpcTransport, StreamTransport};
use metis_platform::native::{
    WebViewConfig, WebViewEvent, WebViewHostEvent, WebViewSurface, WindowConfig, WindowEvent,
    WindowVisibility,
};
use serde::{Deserialize, Serialize};
use std::io::{stdin, stdout};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

const INITIAL_WIDTH: u32 = 1024;
const INITIAL_HEIGHT: u32 = 768;
const EVENT_WAIT: Duration = Duration::from_millis(250);
const MAX_PATIENT_ID_BYTES: usize = 128;
const MAX_PACKAGE_ATTEMPTS: u32 = 8;
const ESCAPE_KEY: u32 = 0x1b;

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'self'; style-src 'self'; img-src 'none'; font-src 'none'; media-src 'none'; connect-src 'none'; object-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; manifest-src 'none'; form-action 'none'; base-uri 'none'">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Metis WebView2 form</title>
  <link rel="stylesheet" href="./styles.css">
</head>
<body>
  <main>
    <h1>Metis clinical calculation</h1>
    <p id="host-status" role="status" aria-live="polite">Waiting for the host bridge.</p>
    <form id="calculation" novalidate>
      <label>Patient reference <input id="patient-id" name="patient_id" value="demo" maxlength="128" autocomplete="off" required></label>
      <label>Weight (kg) <input id="weight" name="weight_kg" type="number" min="0" step="any" value="60" required></label>
      <label>Concentration (mg/mL) <input id="concentration" name="concentration_mg_ml" type="number" min="0" step="any" value="2" required></label>
      <label>Target dose (mcg/kg/min) <input id="dose" name="target_dose_mcg_kg_min" type="number" min="0" step="any" value="0.2" required></label>
      <button type="submit">Submit calculation</button>
    </form>
    <p id="result" role="status" aria-live="polite">No calculation submitted.</p>
  </main>
  <script src="./app.js" defer></script>
</body>
</html>
"#;

const STYLES_CSS: &str = r":root { color-scheme: dark; font-family: system-ui, sans-serif; background: #0f172a; color: #e2e8f0; }
body { margin: 0; min-width: 320px; }
main { box-sizing: border-box; width: min(100% - 2rem, 52rem); margin: 0 auto; padding: 2rem 0; }
h1 { color: #67e8f9; }
form { display: grid; gap: 1rem; padding: 1.25rem; border: 1px solid #334155; border-radius: 0.75rem; background: #1e293b; }
label { display: grid; gap: 0.35rem; color: #bae6fd; }
input { box-sizing: border-box; min-height: 2.75rem; border: 1px solid #64748b; border-radius: 0.4rem; background: #0f172a; color: #f8fafc; padding: 0.65rem; font: inherit; }
button { min-height: 2.75rem; border: 0; border-radius: 0.4rem; background: #0891b2; color: #ecfeff; padding: 0.7rem 1rem; font: inherit; font-weight: 700; }
button:disabled { background: #64748b; cursor: not-allowed; }
input:focus-visible, button:focus-visible { outline: 3px solid #facc15; outline-offset: 2px; }
#host-status, #result { min-height: 1.5rem; color: #bae6fd; }
";

const APP_JS: &str = r"const form = document.getElementById('calculation');
const status = document.getElementById('host-status');
const result = document.getElementById('result');
const bridge = window.chrome && window.chrome.webview;

function showError(message) {
  result.textContent = message;
  form.querySelector('button').disabled = false;
}

if (!bridge) {
  showError('WebView2 bridge is unavailable.');
  status.textContent = 'Host bridge unavailable';
} else {
  status.textContent = 'Host bridge connected; backend authority remains outside the page.';
  bridge.addEventListener('message', (event) => {
    const message = typeof event.data === 'string' ? JSON.parse(event.data) : event.data;
    if (!message || !['result', 'error'].includes(message.type)) return;
    if (message.type === 'result' && message.status === 'success') {
      result.textContent = `Rate ${message.rate_ml_hr} mL/hour; drug ${message.drug_rate_mg_hr} mg/hour; audit ${message.audit_sequence_id}`;
    } else {
      showError(`${message.status}: ${message.message} [0x${message.error_code.toString(16).padStart(4, '0')}]`);
    }
    form.querySelector('button').disabled = false;
  });
  form.addEventListener('submit', (event) => {
    event.preventDefault();
    form.querySelector('button').disabled = true;
    result.textContent = 'Submitting to the supervised backend…';
    bridge.postMessage({
      action: 'submit',
      patient_id: document.getElementById('patient-id').value,
      weight_kg: Number(document.getElementById('weight').value),
      concentration_mg_ml: Number(document.getElementById('concentration').value),
      target_dose_mcg_kg_min: Number(document.getElementById('dose').value),
    });
  });
}
";

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct WebViewRequest {
    action: WebViewAction,
    patient_id: String,
    weight_kg: f64,
    concentration_mg_ml: f64,
    target_dose_mcg_kg_min: f64,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum WebViewAction {
    Submit,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum PageMessage {
    Result {
        status: &'static str,
        audit_sequence_id: u64,
        rate_ml_hr: f64,
        drug_rate_mg_hr: f64,
        is_pediatric: bool,
    },
    Error {
        status: &'static str,
        error_code: u16,
        message: String,
    },
}

struct Package {
    root: PathBuf,
}

impl Package {
    fn create() -> io::Result<Self> {
        let base = std::env::temp_dir();
        let process = std::process::id();
        for attempt in 0..MAX_PACKAGE_ATTEMPTS {
            let root = base.join(format!("metis-webview-{process}-{attempt}"));
            match fs::create_dir(&root) {
                Ok(()) => {
                    let result = (|| {
                        fs::write(root.join("index.html"), INDEX_HTML)?;
                        fs::write(root.join("styles.css"), STYLES_CSS)?;
                        fs::write(root.join("app.js"), APP_JS)?;
                        Ok(())
                    })();
                    return match result {
                        Ok(()) => Ok(Self { root }),
                        Err(error) => Err(cleanup_error(root, error)),
                    };
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "bounded WebView2 package names are exhausted",
        ))
    }

    fn entry_uri(&self) -> io::Result<String> {
        let entry = self.root.join("index.html").canonicalize()?;
        file_uri(&entry)
    }

    fn cleanup(self) -> io::Result<()> {
        fs::remove_dir_all(self.root)
    }
}

/// Runs the visible `WebView2` form over the supervised private pipe.
pub(crate) fn run(inputs: [String; 3]) -> Result<(), Box<dyn std::error::Error>> {
    let [weight, concentration, dose] = inputs;
    let transport = StreamTransport::new(stdin(), stdout());
    let mut app = FrontendApp::new(transport, INITIAL_WIDTH, INITIAL_HEIGHT)?;
    let pid = std::process::id();
    let mut principal_id = [0; 16];
    principal_id[..4].copy_from_slice(&pid.to_be_bytes());
    app.init(pid, principal_id)?;
    app.set_inputs(
        "demo",
        weight.parse()?,
        concentration.parse()?,
        dose.parse()?,
    )?;

    let package = Package::create()?;
    let uri = match package.entry_uri() {
        Ok(uri) => uri,
        Err(error) => return Err(package_failure(package, error).into()),
    };
    let window = match WindowConfig::with_visibility(
        "Metis WebView2 form",
        INITIAL_WIDTH,
        INITIAL_HEIGHT,
        WindowVisibility::Visible,
    ) {
        Ok(window) => window,
        Err(error) => return Err(package_failure(package, error).into()),
    };
    let config = match WebViewConfig::new(uri) {
        Ok(config) => config,
        Err(error) => return Err(package_failure(package, error).into()),
    };
    let mut surface = match WebViewSurface::new(&window, config) {
        Ok(surface) => surface,
        Err(error) => return Err(package_failure(package, error).into()),
    };
    eprintln!("webview_frontend_pid={pid} window={INITIAL_WIDTH}x{INITIAL_HEIGHT}");
    let result = run_event_loop(&mut app, &mut surface);
    finish(result, &mut surface, package)
}

fn run_event_loop<T: IpcTransport>(
    app: &mut FrontendApp<T>,
    surface: &mut WebViewSurface,
) -> Result<()> {
    loop {
        let events = surface.wait_events(EVENT_WAIT)?;
        for event in events {
            match event {
                WebViewHostEvent::Window(
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyDown {
                        virtual_key: ESCAPE_KEY,
                        ..
                    },
                ) => {
                    surface.close()?;
                    return Ok(());
                }
                WebViewHostEvent::Window(WindowEvent::Destroyed) => return Ok(()),
                WebViewHostEvent::Window(WindowEvent::Resized { width, height }) => {
                    surface.resize(width, height)?;
                }
                WebViewHostEvent::WebView(WebViewEvent::Message { json, .. }) => {
                    handle_message(app, surface, &json)?;
                }
                WebViewHostEvent::WebView(WebViewEvent::NavigationCompleted {
                    success: false,
                    ..
                }) => {
                    post_message(
                        surface,
                        &PageMessage::Error {
                            status: "navigation_failed",
                            error_code: ErrorCode::NavigationDenied as u16,
                            message: "Packaged page navigation failed".to_owned(),
                        },
                    )?;
                }
                WebViewHostEvent::WebView(WebViewEvent::NewWindowDenied { .. }) => {
                    post_message(
                        surface,
                        &PageMessage::Error {
                            status: "new_window_denied",
                            error_code: ErrorCode::NavigationDenied as u16,
                            message: "New-window navigation is disabled".to_owned(),
                        },
                    )?;
                }
                WebViewHostEvent::WebView(WebViewEvent::MessageRejected { .. }) => {
                    post_message(
                        surface,
                        &PageMessage::Error {
                            status: "message_rejected",
                            error_code: ErrorCode::InvalidOrigin as u16,
                            message: "Message origin is outside the packaged page".to_owned(),
                        },
                    )?;
                }
                WebViewHostEvent::Window(_) | WebViewHostEvent::WebView(_) => {}
            }
        }
    }
}

fn handle_message<T: IpcTransport>(
    app: &mut FrontendApp<T>,
    surface: &mut WebViewSurface,
    json: &str,
) -> Result<()> {
    let Ok(request) = serde_json::from_str::<WebViewRequest>(json) else {
        return post_message(
            surface,
            &PageMessage::Error {
                status: "invalid_request",
                error_code: ErrorCode::MalformedPayload as u16,
                message: "The host bridge request is not valid".to_owned(),
            },
        );
    };
    if request.patient_id.is_empty()
        || request.patient_id.len() > MAX_PATIENT_ID_BYTES
        || request.patient_id.chars().any(char::is_control)
    {
        return post_message(
            surface,
            &PageMessage::Error {
                status: "invalid_request",
                error_code: ErrorCode::PayloadTooLarge as u16,
                message: "Patient reference is empty, oversized or contains controls".to_owned(),
            },
        );
    }
    if !request.weight_kg.is_finite()
        || !request.concentration_mg_ml.is_finite()
        || !request.target_dose_mcg_kg_min.is_finite()
    {
        return post_message(
            surface,
            &PageMessage::Error {
                status: "invalid_request",
                error_code: ErrorCode::NumericInstability as u16,
                message: "Numeric inputs must be finite".to_owned(),
            },
        );
    }
    match request.action {
        WebViewAction::Submit => {
            app.set_inputs(
                &request.patient_id,
                request.weight_kg,
                request.concentration_mg_ml,
                request.target_dose_mcg_kg_min,
            )?;
            if let Err(error) = app.submit_calculation() {
                return post_message(
                    surface,
                    &PageMessage::Error {
                        status: "transport_failed",
                        error_code: error.code as u16,
                        message: error.message,
                    },
                );
            }
            match app.state() {
                FormState::Success(response) => post_message(
                    surface,
                    &PageMessage::Result {
                        status: "success",
                        audit_sequence_id: response.audit_sequence_id,
                        rate_ml_hr: response.rate_ml_hr,
                        drug_rate_mg_hr: response.drug_rate_mg_hr,
                        is_pediatric: response.is_pediatric,
                    },
                ),
                FormState::Rejected(error) => post_message(
                    surface,
                    &PageMessage::Error {
                        status: "rejected",
                        error_code: error.error_code,
                        message: error.message.clone(),
                    },
                ),
                FormState::Failed(error) | FormState::Disconnected(error) => post_message(
                    surface,
                    &PageMessage::Error {
                        status: "failed",
                        error_code: error.code as u16,
                        message: error.message.clone(),
                    },
                ),
                _ => post_message(
                    surface,
                    &PageMessage::Error {
                        status: "failed",
                        error_code: ErrorCode::TransportBroken as u16,
                        message: "The calculation did not reach a terminal state".to_owned(),
                    },
                ),
            }
        }
    }
}

fn post_message(surface: &mut WebViewSurface, message: &PageMessage) -> Result<()> {
    let json = serde_json::to_string(message).map_err(|error| {
        MetisError::protocol(
            ErrorCode::MalformedPayload,
            format!("WebView2 response encoding failed: {error}"),
        )
    })?;
    surface.post_json(json)?;
    Ok(())
}

fn file_uri(path: &Path) -> io::Result<String> {
    let text = path.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "WebView2 package path is not valid Unicode",
        )
    })?;
    let normalized = text.replace('\\', "/");
    let normalized = normalized
        .strip_prefix("//?/")
        .map_or(normalized.as_str(), |path| path);
    if normalized.starts_with('/') || normalized.starts_with("UNC/") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "WebView2 package path must use a local drive",
        ));
    }
    let mut uri = String::from("file:///");
    for byte in normalized.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/' | b':') {
            uri.push(char::from(byte));
        } else {
            uri.push('%');
            uri.push(char::from(b"0123456789ABCDEF"[(byte >> 4) as usize]));
            uri.push(char::from(b"0123456789ABCDEF"[(byte & 0x0f) as usize]));
        }
    }
    Ok(uri)
}

fn package_failure(package: Package, error: io::Error) -> io::Error {
    cleanup_error(package.root, error)
}

fn finish(
    result: Result<()>,
    surface: &mut WebViewSurface,
    package: Package,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut errors = Vec::new();
    if let Err(error) = result {
        errors.push(error.to_string());
    }
    if let Err(error) = surface.close() {
        errors.push(format!("WebView2 close failed: {error}"));
    }
    if let Err(error) = package.cleanup() {
        errors.push(format!("WebView2 package cleanup failed: {error}"));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; ").into())
    }
}

fn cleanup_error(path: PathBuf, error: io::Error) -> io::Error {
    match fs::remove_dir_all(path) {
        Ok(()) => error,
        Err(cleanup) => io::Error::new(
            error.kind(),
            format!("{error}; WebView2 package cleanup failed: {cleanup}"),
        ),
    }
}

#[cfg(test)]
mod tests;
