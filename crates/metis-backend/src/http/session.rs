//! Session handshake and expiry for the browser HTTP service.
//!
//! A handshake binds the single retained [`HttpSession`] to the trusted
//! launcher principal. Expiry is measured on the monotonic axis of the service
//! clock, so wall-clock adjustments never extend or shorten a session.

use super::{BrowserHttpService, frame_header, status_for_code};
use crate::fragment::UiFragmentPlugin;
use crate::service::{BackendService, Clock, SESSION_LIFETIME};
use metis_core::error::{ErrorCode, MetisError};
use metis_core::host::HostSessionId;
use metis_core::protocol::{
    HandshakeRequestPayload, HandshakeResponsePayload, MessageType, PROTOCOL_VERSION,
};
use metis_ipc::server::IpcHandler;
use moirai_http::HttpResponse;
use std::io;
use std::time::Duration;

pub(super) struct HttpSession<C> {
    pub(super) service: BackendService<C>,
    pub(super) next_sequence: u64,
    /// Expiry on the monotonic axis of the service clock.
    pub(super) expires_at: Duration,
}

impl<C: Clock + Clone> BrowserHttpService<C> {
    pub(super) fn open_session(&mut self, body: &[u8]) -> io::Result<HttpResponse> {
        let request = match HandshakeRequestPayload::decode(body) {
            Ok(request) => request,
            Err(error) => return self.error_response(status_for_code(error.code), &error),
        };
        let principal = request.principal_id;
        match HostSessionId::new(principal) {
            Ok(session_id) if session_id == self.trusted_context.session_id() => {}
            Ok(_) => {
                return self.error_response(
                    403,
                    &MetisError::capability(
                        ErrorCode::InvalidPrincipal,
                        "HTTP handshake principal is not bound to the trusted launcher session",
                    ),
                );
            }
            Err(error) => return self.error_response(status_for_code(error.code), &error),
        }
        let opened = match self.clock.now() {
            Ok(reading) => reading.monotonic,
            Err(error) => return self.error_response(500, &error),
        };
        self.drop_expired_session(opened);
        if self.session.is_some() {
            return self.error_response(
                409,
                &MetisError::capability(
                    ErrorCode::PrivilegeEscalationAttempt,
                    "HTTP session principal is already active",
                ),
            );
        }
        let mut service = match BackendService::with_trusted_context(
            self.master_key,
            self.envelope,
            self.clock.clone(),
            self.policy.clone(),
            self.trusted_context.clone(),
        ) {
            Ok(service) => service,
            Err(error) => return self.error_response(500, &error),
        };
        if let Err(error) = service.register_plugin(UiFragmentPlugin) {
            return self.error_response(500, &error);
        }
        let header = match frame_header(MessageType::HandshakeReq, 1, body) {
            Ok(header) => header,
            Err(error) => return self.error_response(status_for_code(error.code), &error),
        };
        let (message, response) = match service.handle_request(&header, body) {
            Ok(response) => response,
            Err(error) => return self.error_response(500, &error),
        };
        match message {
            MessageType::HandshakeResp => {
                let decoded = match HandshakeResponsePayload::decode(&response) {
                    Ok(decoded) => decoded,
                    Err(error) => return self.error_response(500, &error),
                };
                if decoded.server_version != PROTOCOL_VERSION
                    || decoded.initial_token.principal_id != principal
                {
                    return self.error_response(
                        500,
                        &MetisError::protocol(
                            ErrorCode::VersionMismatch,
                            "HTTP handshake response violated the session contract",
                        ),
                    );
                }
                let Some(expires_at) = opened.checked_add(SESSION_LIFETIME) else {
                    return self.error_response(
                        500,
                        &MetisError::capability(
                            ErrorCode::CapabilityExpired,
                            "Monotonic clock cannot represent the session expiry",
                        ),
                    );
                };
                self.session = Some(HttpSession {
                    service,
                    next_sequence: 2,
                    expires_at,
                });
                self.binary_response(200, response)
            }
            MessageType::ErrorResp => self.wire_error_response(&response),
            _ => self.error_response(
                500,
                &MetisError::protocol(
                    ErrorCode::UnexpectedMessageType,
                    "HTTP handshake returned an invalid response type",
                ),
            ),
        }
    }

    /// Discards the retained session once `now` reaches its expiry.
    fn drop_expired_session(&mut self, now: Duration) {
        if self
            .session
            .as_ref()
            .is_some_and(|session| session.expires_at <= now)
        {
            self.session = None;
        }
    }
}
