//! Parent-process application state and supervised presentation launch.
use crate::{
    entropy,
    invocation::{BrowserResponseDelay, FRONTEND_ROLE},
};
use metis_backend::service::SystemClock;
use metis_backend::{BackendService, clinical::SafetyEnvelope, supervisor::run_session};
use metis_backend::{serve_browser_websocket, serve_browser_websocket_with_response_delay};
use metis_core::host::{HostContext, HostOrigin, HostPolicy, HostSessionId, WindowId};
use metis_core::protocol::TargetCapability;
use moirai_async::net::TcpListener;
use moirai_http::WebSocketConfig;
use std::time::Duration;

pub(crate) fn run(inputs: [String; 3]) -> Result<(), Box<dyn std::error::Error>> {
    // Binary reporter boundary erases errors; no hot-path dispatch.
    let executable = std::env::current_exe()?;
    let mut service = BackendService::new(entropy::session_key()?, SafetyEnvelope::default());
    service.add_target_capability(TargetCapability::PrivateProcessIpc)?;
    let [weight, concentration, dose] = inputs;
    let arguments = [FRONTEND_ROLE.to_owned(), weight, concentration, dose];
    eprintln!("backend_pid={}", std::process::id());
    run_session(&executable, &arguments, &mut service)?;
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
    let service = BackendService::with_trusted_context(
        entropy::session_key()?,
        SafetyEnvelope::default(),
        SystemClock::default(),
        policy,
        context,
    )?;
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

fn principal_hex(principal: [u8; 16]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(32);
    for byte in principal {
        write!(&mut output, "{byte:02x}").expect("invariant: String formatting cannot fail");
    }
    output
}
