//! Authenticated, format-neutral HTTP fragment service over Moirai.
//!
//! This module owns only the presentation boundary. It accepts typed Metis
//! envelopes and returns bounded text or attribute patch bytes; application
//! data formats and domain semantics remain owned by the consuming application.

use crate::clinical::SafetyEnvelope;
use crate::fragment::UiFragmentPlugin;
use crate::service::{BackendService, SystemClock};
use metis_core::crc32;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::host::{HostPolicy, HostSessionId};
use metis_core::protocol::{
    ErrorResponsePayload, FrameHeader, HandshakeRequestPayload, HandshakeResponsePayload,
    MAX_PAYLOAD_SIZE, MessageType, PROTOCOL_VERSION, PluginInvocationPayload,
    PluginInvocationResponsePayload,
};
use metis_ipc::server::IpcHandler;
use moirai_http::{HttpRequest, HttpResponse, HttpServer};
use std::collections::BTreeMap;
use std::io;

/// Maximum authenticated browser sessions retained by one HTTP host.
pub const MAX_HTTP_SESSIONS: usize = 8;

/// Maximum requests served before a demonstration host performs orderly teardown.
pub const MAX_HTTP_REQUESTS: usize = 64;

const SESSION_ROUTE: &str = "/v1/session";
const FRAGMENT_ROUTE: &str = "/v1/fragments";
const HEALTH_ROUTE: &str = "/health";
const BINARY_CONTENT_TYPE: &str = "application/metis";
const ERROR_CONTENT_TYPE: &str = "application/metis-error";
const TEXT_CONTENT_TYPE: &str = "text/plain; charset=utf-8";

struct HttpSession {
    service: BackendService<SystemClock>,
    next_sequence: u64,
}

/// Bounded HTTP application state for one exact browser origin.
pub struct BrowserHttpService {
    master_key: [u8; 32],
    envelope: SafetyEnvelope,
    policy: HostPolicy,
    sessions: BTreeMap<[u8; 16], HttpSession>,
}

impl BrowserHttpService {
    /// Creates a format-neutral service with a deny-by-default origin policy.
    #[must_use]
    pub fn new(master_key: [u8; 32], envelope: SafetyEnvelope, policy: HostPolicy) -> Self {
        Self {
            master_key,
            envelope,
            policy,
            sessions: BTreeMap::new(),
        }
    }

    /// Returns the number of retained authenticated sessions.
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Produces one bounded response for a validated Moirai HTTP request.
    ///
    /// The request origin is checked before route dispatch. Only the three
    /// closed routes in this module are reachable; application payloads remain
    /// binary typed envelopes and never become HTML or script.
    ///
    /// # Errors
    /// Returns an I/O error only if constructing the bounded response headers
    /// fails. Protocol and authorization failures become typed HTTP error
    /// responses so the client can classify them without parsing diagnostics.
    pub fn respond(&mut self, request: &HttpRequest) -> io::Result<HttpResponse> {
        if let Err(error) = self.policy.observe_origin(request.header("origin")) {
            return self.error_response(403, &error);
        }

        match (request.method(), request.target()) {
            ("GET", HEALTH_ROUTE) => self.text_response(200, b"metis-http-ready\n".to_vec()),
            ("POST", SESSION_ROUTE) => self.open_session(request.body()),
            ("POST", FRAGMENT_ROUTE) => self.invoke_fragment(request.body()),
            ("OPTIONS", SESSION_ROUTE | FRAGMENT_ROUTE | HEALTH_ROUTE) => self.preflight_response(),
            ("GET" | "POST", SESSION_ROUTE | FRAGMENT_ROUTE | HEALTH_ROUTE) => self.error_response(
                405,
                &MetisError::protocol(
                    ErrorCode::UnexpectedMessageType,
                    "HTTP method is not admitted for this route",
                ),
            ),
            _ => self.error_response(
                404,
                &MetisError::protocol(ErrorCode::MalformedPayload, "HTTP route is not admitted"),
            ),
        }
    }

    fn open_session(&mut self, body: &[u8]) -> io::Result<HttpResponse> {
        let request = match HandshakeRequestPayload::decode(body) {
            Ok(request) => request,
            Err(error) => return self.error_response(status_for_code(error.code), &error),
        };
        let principal = request.principal_id;
        if self.sessions.contains_key(&principal) {
            return self.error_response(
                409,
                &MetisError::capability(
                    ErrorCode::PrivilegeEscalationAttempt,
                    "HTTP session principal is already active",
                ),
            );
        }
        if self.sessions.len() >= MAX_HTTP_SESSIONS {
            return self.error_response(
                503,
                &MetisError::transport(ErrorCode::QueueFull, "HTTP session capacity is exhausted"),
            );
        }

        let session_id = match HostSessionId::new(principal) {
            Ok(session_id) => session_id,
            Err(error) => return self.error_response(status_for_code(error.code), &error),
        };
        let context = self.policy.context_for(session_id);
        let mut service = match BackendService::with_trusted_context(
            self.master_key,
            self.envelope,
            SystemClock::default(),
            self.policy.clone(),
            context,
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
                self.sessions.insert(
                    principal,
                    HttpSession {
                        service,
                        next_sequence: 2,
                    },
                );
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

    fn invoke_fragment(&mut self, body: &[u8]) -> io::Result<HttpResponse> {
        let invocation = match PluginInvocationPayload::decode(body) {
            Ok(invocation) => invocation,
            Err(error) => return self.error_response(status_for_code(error.code), &error),
        };
        let principal = invocation.token().principal_id;
        let Some(session) = self.sessions.get_mut(&principal) else {
            return self.error_response(
                401,
                &MetisError::capability(
                    ErrorCode::MissingCapability,
                    "HTTP fragment request has no authenticated session",
                ),
            );
        };
        let sequence = session.next_sequence;
        session.next_sequence = match sequence.checked_add(1) {
            Some(next) => next,
            None => {
                return self.error_response(
                    500,
                    &MetisError::protocol(
                        ErrorCode::ReplayDetected,
                        "HTTP session sequence exhausted",
                    ),
                );
            }
        };
        let header = match frame_header(MessageType::PluginInvokeReq, sequence, body) {
            Ok(header) => header,
            Err(error) => return self.error_response(status_for_code(error.code), &error),
        };
        let (message, response) = match session.service.handle_request(&header, body) {
            Ok(response) => response,
            Err(error) => return self.error_response(500, &error),
        };
        match message {
            MessageType::PluginInvokeResp => {
                let response = match PluginInvocationResponsePayload::decode(&response) {
                    Ok(response) => response,
                    Err(error) => return self.error_response(500, &error),
                };
                self.binary_response(200, response.body().to_vec())
            }
            MessageType::ErrorResp => self.wire_error_response(&response),
            _ => self.error_response(
                500,
                &MetisError::protocol(
                    ErrorCode::UnexpectedMessageType,
                    "HTTP fragment route returned an invalid response type",
                ),
            ),
        }
    }

    fn binary_response(&self, status: u16, body: Vec<u8>) -> io::Result<HttpResponse> {
        self.response(status, BINARY_CONTENT_TYPE, body)
    }

    fn text_response(&self, status: u16, body: Vec<u8>) -> io::Result<HttpResponse> {
        self.response(status, TEXT_CONTENT_TYPE, body)
    }

    fn preflight_response(&self) -> io::Result<HttpResponse> {
        let mut response = self.response(204, TEXT_CONTENT_TYPE, Vec::new())?;
        response.set_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")?;
        response.set_header("Access-Control-Allow-Headers", "content-type")?;
        Ok(response)
    }

    fn error_response(&self, status: u16, error: &MetisError) -> io::Result<HttpResponse> {
        let payload = ErrorResponsePayload {
            error_code: error.code as u16,
            message: error.code.as_str().to_owned(),
        }
        .encode()
        .map_err(|error| protocol_io_error(&error))?;
        self.response(status, ERROR_CONTENT_TYPE, payload)
    }

    fn wire_error_response(&self, payload: &[u8]) -> io::Result<HttpResponse> {
        let error =
            ErrorResponsePayload::decode(payload).map_err(|error| protocol_io_error(&error))?;
        self.response(
            status_for_wire_code(error.error_code),
            ERROR_CONTENT_TYPE,
            payload.to_vec(),
        )
    }

    fn response(&self, status: u16, content_type: &str, body: Vec<u8>) -> io::Result<HttpResponse> {
        let mut response = HttpResponse::new(status, body)?;
        response.set_header("Content-Type", content_type)?;
        response.set_header("Cache-Control", "no-store")?;
        response.set_header(
            "Access-Control-Allow-Origin",
            self.policy.allowed_origin().as_str(),
        )?;
        response.set_header("Vary", "Origin")?;
        Ok(response)
    }
}

/// Serves a bounded number of local HTTP requests and then closes the listener.
///
/// The finite request budget is deliberate for the demonstration executable:
/// it gives teardown a clear owner and prevents a sample process from becoming
/// an unbounded background service. A production deployment would supply its
/// own process lifecycle and shutdown signal around the same application.
///
/// # Errors
/// Returns a typed transport or response-construction failure. A malformed,
/// timed-out, or disconnected connection is terminal for this invocation.
pub async fn serve_browser_http(
    server: HttpServer,
    mut application: BrowserHttpService,
    max_requests: usize,
) -> Result<()> {
    if max_requests == 0 || max_requests > MAX_HTTP_REQUESTS {
        return Err(MetisError::transport(
            ErrorCode::QueueFull,
            "HTTP request budget is outside the bounded demonstration range",
        ));
    }
    for _ in 0..max_requests {
        let connection = server
            .accept()
            .await
            .map_err(|error| http_io_error(&error))?;
        let (request, connection) = connection
            .read_request()
            .await
            .map_err(|error| http_io_error(&error))?;
        let response = application.respond(&request).map_err(|_| {
            MetisError::transport(ErrorCode::IoError, "HTTP response construction failed")
        })?;
        connection
            .write_response(response)
            .await
            .map_err(|error| http_io_error(&error))?;
    }
    Ok(())
}

fn frame_header(message_type: MessageType, sequence: u64, payload: &[u8]) -> Result<FrameHeader> {
    if payload.len() > MAX_PAYLOAD_SIZE {
        return Err(MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "HTTP envelope exceeds the Metis payload bound",
        ));
    }
    Ok(FrameHeader {
        msg_type: message_type,
        sequence_id: sequence,
        payload_crc32: crc32(payload),
        payload_len: u32::try_from(payload.len()).map_err(|_| {
            MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "HTTP envelope length exceeds its wire field",
            )
        })?,
    })
}

fn status_for_code(code: ErrorCode) -> u16 {
    status_for_wire_code(code as u16)
}

fn status_for_wire_code(code: u16) -> u16 {
    if code == ErrorCode::PayloadTooLarge as u16 {
        413
    } else if code == ErrorCode::Timeout as u16 {
        408
    } else if (0x2000..=0x2fff).contains(&code) {
        403
    } else if (0x4000..=0x4fff).contains(&code) {
        503
    } else {
        400
    }
}

fn protocol_io_error(error: &MetisError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.code.as_str())
}

fn http_io_error(error: &io::Error) -> MetisError {
    let code = match error.kind() {
        io::ErrorKind::InvalidData | io::ErrorKind::InvalidInput => ErrorCode::MalformedPayload,
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => ErrorCode::Timeout,
        io::ErrorKind::UnexpectedEof => ErrorCode::FrameTruncated,
        io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset => ErrorCode::ConnectionClosed,
        _ => ErrorCode::TransportBroken,
    };
    MetisError::transport(code, "HTTP transport terminated")
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
