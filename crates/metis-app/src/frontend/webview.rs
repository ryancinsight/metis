//! Windows `WebView2` host for the supervised form workflow.

use super::webview_protocol::{
    PageMessage, WebViewAction, WebViewRequest, is_permission_probe_complete,
};
use crate::invocation::WebViewTheme;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_frontend::{FormState, FrontendApp};
use metis_ipc::{IpcTransport, StreamTransport};
use metis_platform::native::{
    WebViewConfig, WebViewEvent, WebViewHostEvent, WebViewSurface, WindowConfig, WindowEvent,
    WindowVisibility,
};
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

/// Pins the `WebView2` browser's device scale factor to the display scale.
///
/// Chromium maps physical pointer input to CSS coordinates with its device
/// scale factor while the host rasterizes the page at the monitor scale. On
/// a display scaled above 96 dpi the two mappings disagree inside a
/// process-unaware host, so a trusted click on a rendered element hit-tests
/// away from the page's layout (measured 2026-10-12: the proportionally
/// scaled submit point hit-tested the packaged page at `dpi/96` times the
/// client point). `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` carries
/// `--force-device-scale-factor`, which pins the browser's input and
/// rasterization mapping to one scale and restores a 1:1 input mapping over
/// the rendered layout. `WebView2` reads the variable only when it creates
/// the browser environment, so it must be in place before
/// [`WebViewSurface::new`].
#[cfg(windows)]
#[expect(
    unsafe_code,
    reason = "Minimal user32 DPI probe and loader env boundary"
)]
fn force_webview_input_scale() {
    use std::ffi::c_void;

    // Windows pseudo-handle for DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2.
    const PER_MONITOR_AWARE_V2: *mut c_void = std::ptr::without_provenance_mut(usize::MAX - 3);
    const VARIABLE: &str = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";

    #[link(name = "user32")]
    unsafe extern "system" {
        fn SetThreadDpiAwarenessContext(context: *mut c_void) -> *mut c_void;
        fn GetDpiForSystem() -> u32;
    }

    // SAFETY: the requested context is an operating-system constant and the
    // call affects only the current thread; the prior context is restored on
    // every return path below.
    let previous = unsafe { SetThreadDpiAwarenessContext(PER_MONITOR_AWARE_V2) };
    if previous.is_null() {
        return;
    }
    // SAFETY: GetDpiForSystem only reads the thread context set above.
    let dpi = unsafe { GetDpiForSystem() };
    // SAFETY: the context handle returned above belongs to this thread and
    // remains valid for restoration here.
    unsafe { SetThreadDpiAwarenessContext(previous) };

    let scale = if dpi == 0 { 1.0 } else { f64::from(dpi) / 96.0 };
    if (scale - 1.0).abs() <= f64::EPSILON {
        return;
    }
    let argument = format!("--force-device-scale-factor={scale:.4}");
    let existing = std::env::var(VARIABLE).unwrap_or_default();
    let joined = if existing.is_empty() {
        argument
    } else {
        format!("{existing} {argument}")
    };
    // SAFETY: this runs on the single window-creating thread before this
    // process spawns any other reader of the environment, so the write
    // cannot race a concurrent environment read.
    unsafe { std::env::set_var(VARIABLE, joined) };
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
    force_webview_input_scale();
    let mut surface = match WebViewSurface::new(&window, config) {
        Ok(surface) => surface,
        Err(error) => return Err(package_failure(package, error).into()),
    };
    eprintln!("webview_frontend_pid={pid} window={INITIAL_WIDTH}x{INITIAL_HEIGHT}");
    let result = run_event_loop(
        &mut app,
        &mut surface,
        capture_output,
        capture_output.map(|_| {
            if matches!(page, Page::PermissionProbe) {
                CaptureMode::PermissionProbe
            } else {
                CaptureMode::FirstFrame
            }
        }),
    );
    finish(result, &mut surface, package)
}

#[derive(Clone, Copy)]
enum CaptureMode {
    FirstFrame,
    PermissionProbe,
}

fn run_event_loop<T: IpcTransport>(
    app: &mut FrontendApp<T>,
    surface: &mut WebViewSurface,
    capture_output: Option<&Path>,
    capture_mode: Option<CaptureMode>,
) -> Result<()> {
    let mut permission_probe_complete = false;
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
                    permission_probe_complete |= is_permission_probe_complete(&json);
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
        let capture_ready = capture_mode.is_some_and(|mode| match mode {
            CaptureMode::FirstFrame => true,
            CaptureMode::PermissionProbe => permission_probe_complete,
        });
        if capture_ready && let Some(output) = capture_output {
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
            surface.close()?;
            return Ok(());
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
    if is_permission_probe_complete(json) {
        return Ok(());
    }
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
