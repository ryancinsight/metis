//! Native owner of the shared Rust page protocol and supervised backend boundary.
//!
//! Ready/capture/stop waits pump the actual native event loop under finite
//! provider deadlines. Backend submission preserves `FrontendApp`'s synchronous
//! IPC contract; backend-in-flight pumping/backpressure is a separate increment
//! requiring cancellable transport ownership, not a detached worker.

mod assets;
mod cleanup;
mod event_loop;
mod package;
mod requests;
mod scale;

use crate::invocation::WebViewTheme;
use metis_core::error::Result;
use metis_frontend::FrontendApp;
use metis_ipc::StreamTransport;
use metis_platform::native::{WebViewSurface, WindowConfig, WindowVisibility};
use metis_web::embedded::PageRole;
use package::{Package, package_failure};
use std::{
    io::{stdin, stdout},
    path::Path,
};

const INITIAL_WIDTH: u32 = 1024;
const INITIAL_HEIGHT: u32 = 768;
const MAX_PATIENT_ID_BYTES: usize = 128;

pub(crate) fn run(inputs: [String; 3]) -> Result<(), Box<dyn std::error::Error>> {
    run_with_page(inputs, PageRole::Form, None, None)
}
pub(crate) fn run_permission_probe(inputs: [String; 3]) -> Result<(), Box<dyn std::error::Error>> {
    run_with_page(inputs, PageRole::PermissionProbe, None, None)
}
pub(crate) fn run_permission_probe_capture(
    inputs: [String; 3],
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    run_with_page(inputs, PageRole::PermissionProbe, Some(output), None)
}
pub(crate) fn run_theme_capture(
    inputs: [String; 3],
    output: &Path,
    theme: WebViewTheme,
) -> Result<(), Box<dyn std::error::Error>> {
    run_with_page(inputs, PageRole::Form, Some(output), Some(theme))
}

fn run_with_page(
    inputs: [String; 3],
    role: PageRole,
    capture: Option<&Path>,
    theme: Option<WebViewTheme>,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .json()
        .without_time()
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
        .try_init()
        .map_err(|error| -> Box<dyn std::error::Error> {
            // Condition 1: the subscriber's concrete runtime error is unknown;
            // this reporting boundary erases auto traits without reallocating.
            error
        })?;
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

    let package = Package::create(role, theme)?;
    let window = match WindowConfig::with_visibility(
        "Metis WebView2 form",
        INITIAL_WIDTH,
        INITIAL_HEIGHT,
        WindowVisibility::Visible,
    ) {
        Ok(window) => window,
        Err(error) => return Err(package_failure(package, error).into()),
    };
    let config = match package.config() {
        Ok(config) => config,
        Err(error) => return Err(package_failure(package, error).into()),
    };
    let source = config.start_uri().to_owned();
    let identity = package.identity();
    scale::force_webview_input_scale();
    let mut surface = match WebViewSurface::new(&window, config) {
        Ok(surface) => surface,
        Err(error) => return Err(package_failure(package, error).into()),
    };
    eprintln!("webview_frontend_pid={pid} window={INITIAL_WIDTH}x{INITIAL_HEIGHT}");
    let result = event_loop::run(&mut app, &mut surface, identity, source, capture.is_some());
    cleanup::finish(result, surface, package, capture)
}

#[cfg(test)]
mod runtime_tests;
#[cfg(test)]
mod tests;
