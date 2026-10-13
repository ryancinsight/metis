//! Actual document messages drive one admitted native page state.
use super::{Deadline, MAX_WEBVIEW_MESSAGE_BYTES, State, close_destroyed, post, protocol};
use crate::frontend::webview::requests;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_frontend::FrontendApp;
use metis_ipc::IpcTransport;
use metis_platform::native::WebViewSurface;
use metis_web::embedded::{
    CapabilityObservation, PageEvent, PageIdentity, PageRole, ResourceCounts, decode_page_event,
};

impl State {
    pub(super) fn message<T: IpcTransport>(
        &mut self,
        app: &mut FrontendApp<T>,
        surface: &mut WebViewSurface,
        source: &str,
        json: &str,
    ) -> Result<bool> {
        self.page.source(source).map_err(|error| protocol(&error))?;
        let event =
            decode_page_event(json, MAX_WEBVIEW_MESSAGE_BYTES).map_err(|error| protocol(&error))?;
        match event {
            PageEvent::Ready { identity } => {
                self.page
                    .session
                    .ready(identity)
                    .map_err(|error| protocol(&error))?;
                tracing::info!(event = "page_ready", json, source, mount = identity.mount().get(), role = identity.role().as_str(), module = %identity.module_digest(), "Actual Rust page ready");
                self.deadline = (identity.role() == PageRole::PermissionProbe)
                    .then(|| Deadline::new("real permission probe"));
                for command in self.page.denial_commands() {
                    post(surface, &command)?;
                }
            }
            PageEvent::Submit {
                identity,
                request,
                inputs,
            } => {
                self.page
                    .session
                    .begin_request(identity, request)
                    .map_err(|error| protocol(&error))?;
                tracing::info!(
                    event = "page_submit",
                    json,
                    source,
                    mount = identity.mount().get(),
                    request = request.get(),
                    "Actual page request admitted"
                );
                let command = requests::submit(app, identity, request, &inputs);
                post(surface, &command)?;
                self.page
                    .session
                    .complete_request(identity, request)
                    .map_err(|error| protocol(&error))?;
                tracing::info!(
                    event = "page_request_completed",
                    mount = identity.mount().get(),
                    request = request.get(),
                    ?command,
                    "Actual backend terminal reply posted"
                );
            }
            PageEvent::ProbeCompleted {
                identity,
                observations,
                resources,
            } => {
                self.probe(identity, *observations, resources, source, json)?;
            }
            PageEvent::CloseRequested { identity } => {
                if identity != self.page.session.identity() {
                    return Err(protocol(&std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Close request has a stale page identity",
                    )));
                }
                self.begin_stop(surface)?;
            }
            PageEvent::Stopped {
                identity,
                resources,
            } => {
                self.page
                    .session
                    .stopped(identity, resources)
                    .map_err(|error| protocol(&error))?;
                tracing::info!(
                    event = "page_stopped",
                    json,
                    source,
                    mount = identity.mount().get(),
                    ?resources,
                    "Actual Rust stop acknowledgement accepted"
                );
                close_destroyed(surface, &mut self.page)?;
                return Ok(true);
            }
            PageEvent::Failed { identity, message } => {
                if identity != self.page.session.identity() {
                    return Err(protocol(&std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Failure has a stale page identity",
                    )));
                }
                tracing::error!(event = "page_failed", json, source, mount = identity.mount().get(), message = %message, "Actual Rust page failure received");
                return Err(MetisError::protocol(
                    ErrorCode::TransportBroken,
                    format!("Rust page failed: {message}"),
                ));
            }
        }
        Ok(false)
    }

    fn probe(
        &mut self,
        identity: PageIdentity,
        observations: [CapabilityObservation; 4],
        resources: ResourceCounts,
        source: &str,
        json: &str,
    ) -> Result<()> {
        tracing::info!(
            event = "page_probe_observed",
            json,
            source,
            mount = identity.mount().get(),
            "Actual browser outcome record received before native admission"
        );
        self.page
            .probe(identity, observations, resources)
            .map_err(|error| protocol(&error))?;
        let reconciled = self.page.probe_capture_ready();
        tracing::info!(
            event = "page_probe_completed",
            json,
            source,
            mount = identity.mount().get(),
            ?resources,
            reconciled,
            "Actual browser outcomes admitted"
        );
        if reconciled {
            self.deadline = None;
        }
        Ok(())
    }
}
