//! Exact shared-protocol lifecycle, native evidence and bounded owner-thread waits.

mod dispatch;
mod messages;

use crate::frontend::webview_admission::NativePage;
use crate::frontend::webview_command_observation::{self, Receiver};
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_frontend::FrontendApp;
use metis_ipc::IpcTransport;
use metis_platform::native::{
    MAX_WEBVIEW_MESSAGE_BYTES, MAX_WEBVIEW_WAIT_MILLISECONDS, WebViewHostEvent, WebViewSurface,
    WindowEvent,
};
use metis_web::embedded::{PageCommand, PageIdentity, PageRole, PageStage, encode_page_command};
use std::time::{Duration, Instant};

const EVENT_WAIT: Duration = Duration::from_millis(250);
const ESCAPE_KEY: u32 = 0x1b;

struct Deadline {
    at: Instant,
    operation: &'static str,
}
impl Deadline {
    fn new(operation: &'static str) -> Self {
        Self {
            at: Instant::now() + Duration::from_millis(u64::from(MAX_WEBVIEW_WAIT_MILLISECONDS)),
            operation,
        }
    }
    fn remaining(&self) -> Result<Duration> {
        let wait = self.at.saturating_duration_since(Instant::now());
        if wait.is_zero() {
            return Err(MetisError::protocol(
                ErrorCode::Timeout,
                format!("WebView2 {} deadline expired", self.operation),
            ));
        }
        Ok(wait.min(EVENT_WAIT))
    }
}

struct State {
    page: NativePage,
    deadline: Option<Deadline>,
    captured: Option<Vec<u8>>,
    command_observation: Option<Receiver>,
}
impl State {
    fn begin_stop(&mut self, surface: &mut WebViewSurface) -> Result<()> {
        if self.page.session.stage() == PageStage::Stopping {
            return Ok(());
        }
        let identity = self.page.session.identity();
        self.page
            .session
            .begin_stop(identity)
            .map_err(|error| protocol(&error))?;
        post(surface, &PageCommand::Stop { identity })?;
        self.deadline = Some(Deadline::new("Rust page stop acknowledgement"));
        Ok(())
    }

    fn capture_ready(&self) -> bool {
        self.page.session.stage() == PageStage::Ready
            && (self.page.session.identity().role() == PageRole::Form
                || self.page.probe_capture_ready())
    }
}

pub(super) fn run<T: IpcTransport>(
    app: &mut FrontendApp<T>,
    surface: &mut WebViewSurface,
    identity: PageIdentity,
    source: String,
    capture_requested: bool,
) -> Result<Option<Vec<u8>>> {
    let mut state = State {
        page: NativePage::new(identity, source).map_err(|error| protocol(&error))?,
        deadline: Some(Deadline::new("Rust page readiness")),
        captured: None,
        command_observation: webview_command_observation::selected(identity.role())
            .then(Receiver::new),
    };
    loop {
        let wait = state
            .deadline
            .as_ref()
            .map_or(Ok(EVENT_WAIT), Deadline::remaining)?;
        for event in surface.wait_events(wait)? {
            if state.dispatch(app, surface, event)? {
                return Ok(state.captured);
            }
        }
        if state.capture_ready() && capture_requested && state.captured.is_none() {
            let bytes = surface.capture_preview_png()?;
            tracing::info!(
                mount = identity.mount().get(),
                bytes = bytes.len(),
                "Actual ready WebView2 frame captured"
            );
            // Bytes remain owned through stop, actual destruction, native
            // owner drop and profile/package cleanup before publication.
            state.captured = Some(bytes);
            state.begin_stop(surface)?;
        }
    }
}

pub(super) fn close_destroyed(surface: &mut WebViewSurface, page: &mut NativePage) -> Result<()> {
    surface.close()?;
    // PAL preserves the actual final window event after synchronous close.
    // is_closed alone cannot advance the shared controller-destruction state.
    if !surface
        .poll_events()?
        .iter()
        .any(|event| matches!(event, WebViewHostEvent::Window(WindowEvent::Destroyed)))
    {
        return Err(MetisError::protocol(
            ErrorCode::ConnectionClosed,
            "Native close has no actual window destruction observation",
        ));
    }
    page.session
        .observe_destroyed()
        .map_err(|error| protocol(&error))?;
    tracing::info!(
        event = "page_destroyed",
        mount = page.session.identity().mount().get(),
        "Actual native controller destruction observed"
    );
    Ok(())
}

pub(super) fn post(surface: &mut WebViewSurface, command: &PageCommand) -> Result<()> {
    let json = encode_page_command(command, MAX_WEBVIEW_MESSAGE_BYTES)
        .map_err(|error| protocol(&error))?;
    surface.post_json(&json)?;
    let event = match command {
        PageCommand::Result { .. } | PageCommand::Error { .. } => "page_terminal",
        PageCommand::Stop { .. } => "page_stop_requested",
        PageCommand::Mount { .. } => "page_mount_requested",
        PageCommand::PermissionDenied { .. } => "page_permission_reconciled",
    };
    tracing::info!(event, json = %json, mount = command.identity().mount().get(), "Actual native command posted");
    Ok(())
}

fn protocol(error: &std::io::Error) -> MetisError {
    MetisError::protocol(ErrorCode::MalformedPayload, error.to_string())
}
