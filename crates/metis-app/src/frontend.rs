//! One headless form submission over inherited pipes; stdout is wire bytes only.
use crate::invocation::WebViewTheme;
use metis_frontend::{FormState, FrontendApp};
use metis_ipc::transport::StreamTransport;
use std::{
    io::{stdin, stdout},
    path::Path,
};

#[cfg(windows)]
mod native;
#[cfg(windows)]
mod webview;

/// Runs the visible native host on supported desktop targets.
pub(crate) fn run_native(inputs: [String; 3]) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        native::run(inputs)
    }
    #[cfg(not(windows))]
    {
        let _ = inputs;
        Err("the native frontend role requires Windows".into())
    }
}

/// Runs the visible Windows `WebView2` host over the supervised pipe.
pub(crate) fn run_webview(inputs: [String; 3]) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        webview::run(inputs)
    }
    #[cfg(not(windows))]
    {
        let _ = inputs;
        Err("the WebView2 frontend role requires Windows".into())
    }
}

/// Runs the visible Windows `WebView2` permission-denial probe.
pub(crate) fn run_webview_permission_probe(
    inputs: [String; 3],
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        webview::run_permission_probe(inputs)
    }
    #[cfg(not(windows))]
    {
        let _ = inputs;
        Err("the WebView2 permission-probe frontend requires Windows".into())
    }
}

/// Runs the visible Windows `WebView2` permission probe and saves its page.
pub(crate) fn run_webview_permission_probe_capture(
    output: &Path,
    inputs: [String; 3],
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        webview::run_permission_probe_capture(inputs, output)
    }
    #[cfg(not(windows))]
    {
        let _ = (output, inputs);
        Err("the WebView2 permission-probe capture frontend requires Windows".into())
    }
}

/// Runs the visible `WebView2` form and captures one bounded presentation mode.
pub(crate) fn run_webview_theme_capture(
    output: &Path,
    theme: WebViewTheme,
    inputs: [String; 3],
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        webview::run_theme_capture(inputs, output, theme)
    }
    #[cfg(not(windows))]
    {
        let _ = (output, theme, inputs);
        Err("the WebView2 theme capture frontend requires Windows".into())
    }
}

pub(crate) fn run(inputs: [String; 3]) -> Result<(), Box<dyn std::error::Error>> {
    // Binary error reporter is the non-hot type-erasure boundary.
    let [weight, concentration, dose] = inputs;
    let transport = StreamTransport::new(stdin(), stdout());
    let mut app = FrontendApp::new(transport, 800, 600)?;
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
    app.submit_calculation()?;
    match app.state() {
        FormState::Success(response) => eprintln!(
            "frontend_pid={pid} rate_ml_hr={} drug_rate_mg_hr={} audit_sequence={}",
            response.rate_ml_hr, response.drug_rate_mg_hr, response.audit_sequence_id
        ),
        FormState::Rejected(error) => {
            return Err(format!(
                "Backend rejected request [0x{:04X}]: {}",
                error.error_code, error.message
            )
            .into());
        }
        state => return Err(format!("Submission completed without a result: {state:?}").into()),
    }
    Ok(())
}
