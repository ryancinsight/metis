//! Asynchronous request dispatch over a bounded Moirai WebSocket stream.

use crate::frame::read_frame;
use crate::server::{FailureContext, IpcHandler, dispatch_request};
use crate::transport::check_wire_size;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::build_frame;
use moirai_async::io::{AsyncRead, AsyncWrite};
use moirai_http::WebSocketStream;
use std::io;

/// One ordered Metis IPC session carried by a message-oriented WebSocket.
///
/// The server owns one receive/send stream and consumes each request sequence
/// before invoking the handler. A connection is therefore single-consumer and
/// replay-safe just like the synchronous pipe server.
pub struct AsyncIpcServer<S> {
    stream: WebSocketStream<S>,
    last_sequence: u64,
}

impl<S> AsyncIpcServer<S> {
    /// Starts a session over an already-authenticated WebSocket upgrade.
    #[must_use]
    pub const fn new(stream: WebSocketStream<S>) -> Self {
        Self {
            stream,
            last_sequence: 0,
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncIpcServer<S> {
    /// Processes one request, or returns `false` for a peer close frame.
    ///
    /// The WebSocket layer enforces masking, message bounds, control-frame
    /// rules, and frame deadlines. This layer decodes exactly one Metis wire
    /// frame per WebSocket message and applies the same sequence and response
    /// contracts as [`crate::IpcServer`].
    ///
    /// # Errors
    /// Returns malformed-frame, replay, handler, response, transport, or
    /// deadline failures. A clean WebSocket close is reported as `Ok(false)`.
    pub async fn step<H: IpcHandler>(&mut self, handler: &mut H) -> Result<bool> {
        let bytes = match self.stream.recv_message().await {
            Ok(bytes) => bytes,
            Err(error) if is_peer_close(&error) => return Ok(false),
            Err(error) => {
                let error = websocket_error(&error);
                handler.handle_failure(FailureContext::Receive, error.code)?;
                return Err(error);
            }
        };
        let (header, payload) = match decode_message(&bytes) {
            Ok(message) => message,
            Err(error) => {
                handler.handle_failure(FailureContext::Receive, error.code)?;
                return Err(error);
            }
        };
        let dispatched = dispatch_request(&mut self.last_sequence, handler, header, &payload)?;
        let wire = match build_frame(
            dispatched.message_type,
            dispatched.identity.sequence,
            &dispatched.payload,
        ) {
            Ok(wire) => wire,
            Err(error) => {
                handler
                    .handle_failure(FailureContext::Response(dispatched.identity), error.code)?;
                return Err(error);
            }
        };
        if let Err(error) = check_wire_size(&wire) {
            handler.handle_failure(FailureContext::Response(dispatched.identity), error.code)?;
            return Err(error);
        }
        if let Err(error) = self.stream.send_binary(&wire).await {
            let error = websocket_error(&error);
            handler.handle_failure(FailureContext::Response(dispatched.identity), error.code)?;
            return Err(error);
        }
        Ok(true)
    }

    /// Services requests until the peer closes the session.
    ///
    /// The handler remains synchronous by contract. It runs only after one
    /// bounded WebSocket message has been received, so browser/event-loop
    /// callers use this API only on a native service task.
    ///
    /// # Errors
    /// Returns the first protocol, handler, response, transport, or deadline
    /// failure reported by [`Self::step`].
    pub async fn run<H: IpcHandler>(&mut self, handler: &mut H) -> Result<()> {
        while self.step(handler).await? {}
        Ok(())
    }
}

fn decode_message(bytes: &[u8]) -> Result<(metis_core::protocol::FrameHeader, Vec<u8>)> {
    if bytes.is_empty() {
        return Err(MetisError::protocol(
            ErrorCode::FrameTruncated,
            "Empty WebSocket message cannot contain a Metis frame",
        ));
    }
    let mut wire = bytes;
    let message = read_frame(&mut wire)?;
    if !wire.is_empty() {
        return Err(MetisError::protocol(
            ErrorCode::MalformedPayload,
            "Trailing bytes after WebSocket Metis frame",
        ));
    }
    Ok(message)
}

fn is_peer_close(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::UnexpectedEof
        && error.to_string() == "WebSocket peer closed the connection"
}

fn websocket_error(error: &io::Error) -> MetisError {
    let message = error.to_string();
    let code = match error.kind() {
        io::ErrorKind::InvalidData if message.contains("exceeds configured byte bound") => {
            ErrorCode::PayloadTooLarge
        }
        io::ErrorKind::InvalidData | io::ErrorKind::InvalidInput => ErrorCode::MalformedPayload,
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => ErrorCode::Timeout,
        io::ErrorKind::UnexpectedEof => ErrorCode::FrameTruncated,
        io::ErrorKind::BrokenPipe => ErrorCode::ConnectionClosed,
        _ => ErrorCode::TransportBroken,
    };
    if matches!(
        code,
        ErrorCode::MalformedPayload | ErrorCode::PayloadTooLarge
    ) {
        MetisError::protocol(code, message)
    } else {
        MetisError::transport(code, message)
    }
}
