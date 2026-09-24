//! Parent-process application state and supervised presentation launch.
use crate::{
    entropy,
    invocation::{
        BrowserResponseDelay, FRONTEND_ROLE, NATIVE_FRONTEND_ROLE, WEBVIEW_FRONTEND_ROLE,
        WEBVIEW_PERMISSION_PROBE_CAPTURE_FRONTEND_ROLE, WEBVIEW_PERMISSION_PROBE_FRONTEND_ROLE,
        WEBVIEW_THEME_CAPTURE_FRONTEND_ROLE, WebViewTheme,
    },
};
use metis_backend::UiFragmentPlugin;
use metis_backend::service::SystemClock;
use metis_backend::{
    BackendService, INTERACTIVE_SESSION_DEADLINE, ProcessEnvironment, SESSION_DEADLINE,
    clinical::SafetyEnvelope, supervisor::run_session_with_deadline_and_environment,
};
use metis_backend::{
    BrowserHttpService, MAX_HTTP_REQUESTS, serve_browser_http_with_response_delay,
    serve_browser_websocket, serve_browser_websocket_with_response_delay,
};
use metis_core::host::{HostContext, HostOrigin, HostPolicy, HostSessionId, WindowId};
use metis_core::protocol::TargetCapability;
use moirai_async::net::TcpListener;
use moirai_http::{HttpServer, ServerConfig, WebSocketConfig};
use std::{path::PathBuf, time::Duration};

pub(crate) fn run(inputs: [String; 3]) -> Result<(), Box<dyn std::error::Error>> {
    run_with_mode(inputs, &FrontendMode::Headless)
}

/// Runs the same supervised workflow with the Windows native frontend.
pub(crate) fn run_native(inputs: [String; 3]) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        run_with_mode(inputs, &FrontendMode::NativeWindow)
    }
    #[cfg(not(windows))]
    {
        drop(inputs);
        Err("the native window role requires Windows".into())
    }
}

/// Runs the same supervised workflow with the Windows `WebView2` frontend.
pub(crate) fn run_webview(inputs: [String; 3]) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        run_with_mode(inputs, &FrontendMode::WebView)
    }
    #[cfg(not(windows))]
    {
        drop(inputs);
        Err("the WebView2 role requires Windows".into())
    }
}

/// Runs the supervised `WebView2` permission-denial demonstration.
pub(crate) fn run_webview_permission_probe(
    inputs: [String; 3],
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        run_with_mode(inputs, &FrontendMode::WebViewPermissionProbe)
    }
    #[cfg(not(windows))]
    {
        drop(inputs);
        Err("the WebView2 permission-probe role requires Windows".into())
    }
}

/// Runs the permission probe and saves a WebView2-owned PNG preview.
pub(crate) fn run_webview_permission_probe_capture(
    output: PathBuf,
    inputs: [String; 3],
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        run_with_mode(inputs, &FrontendMode::WebViewPermissionProbeCapture(output))
    }
    #[cfg(not(windows))]
    {
        let _ = (output, inputs);
        Err("the WebView2 permission-probe capture role requires Windows".into())
    }
}

/// Captures one packaged `WebView2` form for a bounded presentation mode.
pub(crate) fn run_webview_theme_capture(
    output: PathBuf,
    theme: WebViewTheme,
    inputs: [String; 3],
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        run_with_mode(inputs, &FrontendMode::WebViewThemeCapture { output, theme })
    }
    #[cfg(not(windows))]
    {
        let _ = (output, theme, inputs);
        Err("the WebView2 theme capture role requires Windows".into())
    }
}

#[cfg_attr(
    not(windows),
    expect(
        dead_code,
        reason = "visible host modes are constructed only on Windows"
    )
)]
enum FrontendMode {
    Headless,
    NativeWindow,
    WebView,
    WebViewPermissionProbe,
    WebViewPermissionProbeCapture(PathBuf),
    WebViewThemeCapture {
        output: PathBuf,
        theme: WebViewTheme,
    },
}

fn run_with_mode(
    inputs: [String; 3],
    mode: &FrontendMode,
) -> Result<(), Box<dyn std::error::Error>> {
    // Binary reporter boundary erases errors; no hot-path dispatch.
    let executable = std::env::current_exe()?;
    let mut service = BackendService::new(entropy::session_key()?, SafetyEnvelope::default());
    service.add_target_capability(TargetCapability::PrivateProcessIpc)?;
    let (frontend_role, native_capability, deadline, capture_output) = match mode {
        FrontendMode::Headless => (FRONTEND_ROLE, None, SESSION_DEADLINE, None),
        FrontendMode::NativeWindow => (
            NATIVE_FRONTEND_ROLE,
            Some(TargetCapability::NativeWindow),
            INTERACTIVE_SESSION_DEADLINE,
            None,
        ),
        FrontendMode::WebView => (
            WEBVIEW_FRONTEND_ROLE,
            Some(TargetCapability::NativeWindow),
            INTERACTIVE_SESSION_DEADLINE,
            None,
        ),
        FrontendMode::WebViewPermissionProbe => (
            WEBVIEW_PERMISSION_PROBE_FRONTEND_ROLE,
            Some(TargetCapability::NativeWindow),
            INTERACTIVE_SESSION_DEADLINE,
            None,
        ),
        FrontendMode::WebViewPermissionProbeCapture(output) => (
            WEBVIEW_PERMISSION_PROBE_CAPTURE_FRONTEND_ROLE,
            Some(TargetCapability::NativeWindow),
            INTERACTIVE_SESSION_DEADLINE,
            Some((output, None)),
        ),
        FrontendMode::WebViewThemeCapture { output, theme } => (
            WEBVIEW_THEME_CAPTURE_FRONTEND_ROLE,
            Some(TargetCapability::NativeWindow),
            INTERACTIVE_SESSION_DEADLINE,
            Some((output, Some(*theme))),
        ),
    };
    if let Some(capability) = native_capability {
        service.add_target_capability(capability)?;
    }
    let [weight, concentration, dose] = inputs;
    let mut arguments = Vec::with_capacity(if capture_output.is_some() { 6 } else { 4 });
    arguments.push(frontend_role.to_owned());
    if let Some((output, theme)) = capture_output {
        arguments.push(output.to_string_lossy().into_owned());
        if let Some(theme) = theme {
            arguments.push(theme.query_value().to_owned());
        }
    }
    arguments.extend([weight, concentration, dose]);
    eprintln!("backend_pid={}", std::process::id());
    let environment = match mode {
        FrontendMode::WebView
        | FrontendMode::WebViewPermissionProbe
        | FrontendMode::WebViewPermissionProbeCapture(_)
        | FrontendMode::WebViewThemeCapture { .. } => ProcessEnvironment::Runtime,
        FrontendMode::Headless | FrontendMode::NativeWindow => ProcessEnvironment::Isolated,
    };
    run_session_with_deadline_and_environment(
        &executable,
        &arguments,
        &mut service,
        deadline,
        environment,
    )?;
    service.ledger().verify_chain()?;
    eprintln!(
        "Metis session completed; {} audit records verified",
        service.ledger().records().len()
    );
    Ok(())
}

pub(crate) fn run_browser_service(
    raw_origin: &str,
    port: u16,
    principal: [u8; 16],
    response_delay: Option<BrowserResponseDelay>,
) -> Result<(), Box<dyn std::error::Error>> {
    let origin = HostOrigin::parse(raw_origin)?;
    let window = WindowId::new(1)?;
    let policy = HostPolicy::new(origin.clone(), window);
    let context = HostContext::new(origin.clone(), window, HostSessionId::new(principal)?);
    let mut service = BackendService::with_trusted_context(
        entropy::session_key()?,
        SafetyEnvelope::default(),
        SystemClock::default(),
        policy,
        context,
    )?;
    service.register_plugin(UiFragmentPlugin)?;
    let listener = moirai_executor::block_on(TcpListener::bind(&format!("127.0.0.1:{port}")))?;
    let address = listener.local_addr()?;
    eprintln!("browser_service_endpoint=ws://{address}/socket");
    eprintln!("browser_service_origin={origin}");
    eprintln!("browser_service_principal={}", principal_hex(principal));
    let (stream, peer) = moirai_executor::block_on(listener.accept())?;
    eprintln!("browser_service_peer={peer}");
    let config = WebSocketConfig::new(
        4096,
        16,
        65_560,
        Duration::from_secs(10),
        Duration::from_secs(30),
    );
    if let Some(response_delay) = response_delay {
        moirai_executor::block_on(serve_browser_websocket_with_response_delay(
            stream,
            config,
            service,
            response_delay.duration(),
        ))?;
    } else {
        moirai_executor::block_on(serve_browser_websocket(stream, config, service))?;
    }
    Ok(())
}

/// Runs the bounded loopback HTTP fragment demonstration.
pub(crate) fn run_http_service(
    raw_origin: &str,
    port: u16,
    principal: [u8; 16],
    response_delay: Option<BrowserResponseDelay>,
) -> Result<(), Box<dyn std::error::Error>> {
    let origin = HostOrigin::parse(raw_origin)?;
    let window = WindowId::new(1)?;
    let policy = HostPolicy::new(origin.clone(), window);
    let server_config = ServerConfig::new(
        16,
        16 * 1024,
        64,
        metis_core::MAX_PAYLOAD_SIZE,
        128 * 1024,
        Duration::from_secs(30),
    )?;
    let server = moirai_executor::block_on(HttpServer::bind(
        &format!("127.0.0.1:{port}"),
        server_config,
    ))?;
    let address = server.local_addr()?;
    eprintln!("browser_http_endpoint=http://{address}");
    eprintln!("browser_http_origin={origin}");
    eprintln!("browser_http_principal={}", principal_hex(principal));
    let application =
        BrowserHttpService::new(entropy::session_key()?, SafetyEnvelope::default(), policy);
    moirai_executor::block_on(serve_browser_http_with_response_delay(
        server,
        application,
        MAX_HTTP_REQUESTS,
        response_delay.map(BrowserResponseDelay::duration),
        |error| eprintln!("browser_http_connection_error={error}"),
    ))?;
    Ok(())
}

fn principal_hex(principal: [u8; 16]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(32);
    for byte in principal {
        write!(&mut output, "{byte:02x}").expect("invariant: String formatting cannot fail");
    }
    output
}
