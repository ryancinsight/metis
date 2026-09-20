#![doc = include_str!("../README.md")]
#![deny(missing_docs)]

mod backend;
mod entropy;
mod frontend;
mod invocation;

use invocation::{Invocation, USAGE};

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // The application reporter is a non-hot error type-erasure boundary.
    // Select the child before constructing any parent authority or secret state.
    match Invocation::parse(std::env::args_os().skip(1))? {
        Invocation::Backend(inputs) => backend::run(inputs),
        Invocation::NativeWindow(inputs) => backend::run_native(inputs),
        Invocation::WebView(inputs) => backend::run_webview(inputs),
        Invocation::Frontend(inputs) => frontend::run(inputs),
        Invocation::NativeFrontend(inputs) => frontend::run_native(inputs),
        Invocation::WebViewFrontend(inputs) => frontend::run_webview(inputs),
        Invocation::WebViewPermissionProbe(inputs) => backend::run_webview_permission_probe(inputs),
        Invocation::WebViewPermissionProbeFrontend(inputs) => {
            frontend::run_webview_permission_probe(inputs)
        }
        Invocation::WebViewPermissionProbeCapture { output, inputs } => {
            backend::run_webview_permission_probe_capture(output, inputs)
        }
        Invocation::WebViewPermissionProbeCaptureFrontend { output, inputs } => {
            frontend::run_webview_permission_probe_capture(&output, inputs)
        }
        Invocation::WebViewThemeCapture {
            output,
            theme,
            inputs,
        } => backend::run_webview_theme_capture(output, theme, inputs),
        Invocation::WebViewThemeCaptureFrontend {
            output,
            theme,
            inputs,
        } => frontend::run_webview_theme_capture(&output, theme, inputs),
        Invocation::BrowserService {
            origin,
            port,
            principal,
            response_delay,
        } => backend::run_browser_service(&origin, port, principal, response_delay),
        Invocation::HttpService {
            origin,
            port,
            principal,
            response_delay,
        } => backend::run_http_service(&origin, port, principal, response_delay),
        Invocation::Help => {
            println!("{USAGE}");
            Ok(())
        }
    }
}
