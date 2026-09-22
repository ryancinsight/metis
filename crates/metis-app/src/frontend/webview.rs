//! Windows `WebView2` host for the supervised form workflow.

use crate::invocation::WebViewTheme;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_frontend::{FormState, FrontendApp};
use metis_ipc::{IpcTransport, StreamTransport};
use metis_platform::native::{
    WebViewConfig, WebViewEvent, WebViewHostEvent, WebViewSurface, WindowConfig, WindowEvent,
    WindowVisibility,
};
use serde::{Deserialize, Serialize};
use std::io::{stdin, stdout};
use std::{fs, path::Path, time::Duration};

mod assets;
mod package;

use package::{Package, Page, package_failure};

const INITIAL_WIDTH: u32 = 1024;
const INITIAL_HEIGHT: u32 = 768;
const EVENT_WAIT: Duration = Duration::from_millis(250);
const MAX_PATIENT_ID_BYTES: usize = 128;
const MAX_PACKAGE_ATTEMPTS: u32 = 8;
const ESCAPE_KEY: u32 = 0x1b;

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
    PermissionDenied {
        status: &'static str,
        error_code: u16,
        permission: String,
        message: String,
        user_initiated: bool,
    },
}

/// Runs the visible `WebView2` form over the supervised private pipe.
pub(crate) fn run(inputs: [String; 3]) -> Result<(), Box<dyn std::error::Error>> {
    run_with_page(inputs, Page::Form, None, None)
}

/// Runs a visible `WebView2` page that requests a denied browser capability.
pub(crate) fn run_permission_probe(inputs: [String; 3]) -> Result<(), Box<dyn std::error::Error>> {
    run_with_page(inputs, Page::PermissionProbe, None, None)
}

/// Runs the permission probe and writes a WebView2-owned PNG preview.
pub(crate) fn run_permission_probe_capture(
    inputs: [String; 3],
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    run_with_page(inputs, Page::PermissionProbe, Some(output), None)
}

/// Runs the packaged form in one requested theme and captures its first frame.
pub(crate) fn run_theme_capture(
    inputs: [String; 3],
    output: &Path,
    theme: WebViewTheme,
) -> Result<(), Box<dyn std::error::Error>> {
    run_with_page(inputs, Page::Form, Some(output), Some(theme))
}

fn run_with_page(
    inputs: [String; 3],
    page: Page,
    capture_output: Option<&Path>,
    initial_theme: Option<WebViewTheme>,
) -> Result<(), Box<dyn std::error::Error>> {
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

    let package = Package::create(page, initial_theme)?;
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
    let result = run_event_loop(
        &mut app,
        &mut surface,
        capture_output,
        initial_theme.is_some(),
    );
    finish(result, &mut surface, package)
}

fn run_event_loop<T: IpcTransport>(
    app: &mut FrontendApp<T>,
    surface: &mut WebViewSurface,
    capture_output: Option<&Path>,
    close_after_capture: bool,
) -> Result<()> {
    let mut captured = false;
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
                WebViewHostEvent::WebView(WebViewEvent::PermissionDenied {
                    permission,
                    user_initiated,
                    ..
                }) => {
                    post_message(
                        surface,
                        &permission_denied_message(permission, user_initiated),
                    )?;
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
        if !captured && let Some(output) = capture_output {
            let bytes = surface.capture_preview_png().map_err(|error| {
                MetisError::protocol(
                    ErrorCode::TransportBroken,
                    format!("WebView2 preview capture failed: {error}"),
                )
            })?;
            fs::write(output, &bytes).map_err(|error| {
                MetisError::protocol(
                    ErrorCode::TransportBroken,
                    format!("WebView2 preview output write failed: {error}"),
                )
            })?;
            eprintln!(
                "webview_preview_path={} webview_preview_bytes={}",
                output.display(),
                bytes.len()
            );
            captured = true;
            if close_after_capture {
                surface.close()?;
                return Ok(());
            }
        }
    }
}

fn permission_denied_message(
    permission: metis_platform::native::WebViewPermission,
    user_initiated: bool,
) -> PageMessage {
    PageMessage::PermissionDenied {
        status: "permission_denied",
        error_code: ErrorCode::PermissionDenied as u16,
        permission: permission.to_string(),
        message: format!("WebView2 denied {permission} access request"),
        user_initiated,
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

#[cfg(test)]
mod tests;
