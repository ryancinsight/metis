//! Browser WebSocket transport backed by Moirai's WebAssembly PAL.

use crate::frame::read_frame;
use crate::transport::{AsyncIpcTransport, check_wire_size};
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{FrameHeader, HEADER_SIZE, MAX_PAYLOAD_SIZE};
use moirai_pal::wasm::{WebReactor, WebSocketLimits, WebSocketOpen, WebSocketReceive, WebTimer};
use moirai_pal::{Interest, RawFd, Reactor};
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

/// Default browser message bound equal to one complete Metis wire frame.
pub const DEFAULT_MESSAGE_BYTES: usize = HEADER_SIZE + MAX_PAYLOAD_SIZE;

/// Default number of complete frames retained by one browser connection.
pub const DEFAULT_QUEUED_MESSAGES: usize = 16;

/// A bounded WebSocket transport for the browser event-loop thread.
pub struct BrowserWebSocketTransport {
    reactor: WebReactor,
    fd: RawFd,
}

impl BrowserWebSocketTransport {
    /// Opens a WebSocket and registers it for event-driven reads and writes.
    ///
    /// `limits` bounds the browser data copied into Rust-owned memory. The
    /// caller must provide an origin-appropriate URL; browser origin policy
    /// remains enforced by the WebSocket implementation and the server's
    /// capability boundary.
    ///
    /// # Errors
    /// Returns a connection, registration, or invalid-limit error.
    pub fn connect(url: &str, limits: WebSocketLimits) -> Result<Self> {
        let mut reactor = WebReactor::new().map_err(|error| map_browser_error(&error))?;
        let fd = reactor
            .create_websocket_with_limits(url, limits)
            .map_err(|error| map_browser_error(&error))?;
        if let Err(register_error) = reactor.register_fd(fd, Interest::READ_WRITE) {
            return match reactor.websocket_close(fd) {
                Ok(()) => Err(map_browser_error(&register_error)),
                Err(close_error) => Err(MetisError::transport(
                    ErrorCode::TransportBroken,
                    format!(
                        "WebSocket registration failed: {register_error}; cleanup failed: {close_error}"
                    ),
                )),
            };
        }
        Ok(Self { reactor, fd })
    }

    /// Opens a connection with a frame-sized message bound and a finite queue.
    ///
    /// # Errors
    /// Returns a connection or registration error.
    pub fn connect_with_defaults(url: &str) -> Result<Self> {
        let limits = WebSocketLimits::new(DEFAULT_MESSAGE_BYTES, DEFAULT_QUEUED_MESSAGES)
            .map_err(|error| map_browser_error(&error))?;
        Self::connect(url, limits)
    }

    /// Opens a connection and waits for the browser WebSocket `OPEN` event.
    ///
    /// Sending before `OPEN` is a browser state-machine error. This constructor
    /// keeps the transport owned while awaiting the provider's
    /// cancellation-safe readiness future and closes the descriptor when the
    /// finite opening deadline expires.
    ///
    /// # Errors
    /// Returns connection, registration, readiness, or timeout errors.
    pub async fn connect_async(
        url: &str,
        limits: WebSocketLimits,
        open_timeout: Duration,
    ) -> Result<Self> {
        if open_timeout.is_zero() {
            return Err(MetisError::transport(
                ErrorCode::Timeout,
                "WebSocket OPEN timeout must be non-zero",
            ));
        }
        let mut transport = Self::connect(url, limits)?;
        let open = transport
            .reactor
            .websocket_open_async(transport.fd)
            .map_err(|error| map_browser_error(&error))?;
        let timer = WebTimer::new(open_timeout).map_err(|error| map_browser_error(&error))?;
        let result = OpenOrTimeout { open, timer }.await;
        if let Err(error) = result {
            let close_result = transport.reactor.websocket_close(transport.fd);
            return match close_result {
                Ok(()) => Err(map_browser_error(&error)),
                Err(close_error) => Err(MetisError::transport(
                    ErrorCode::TransportBroken,
                    format!("WebSocket OPEN failed: {error}; cleanup failed: {close_error}"),
                )),
            };
        }
        Ok(transport)
    }

    /// Opens a connection with the default frame and queue bounds, waiting for
    /// `OPEN` within `open_timeout`.
    ///
    /// # Errors
    /// Returns connection, registration, readiness, or timeout errors.
    pub async fn connect_with_defaults_async(url: &str, open_timeout: Duration) -> Result<Self> {
        let limits = WebSocketLimits::new(DEFAULT_MESSAGE_BYTES, DEFAULT_QUEUED_MESSAGES)
            .map_err(|error| map_browser_error(&error))?;
        Self::connect_async(url, limits, open_timeout).await
    }
}

impl AsyncIpcTransport for BrowserWebSocketTransport {
    fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
        check_wire_size(frame)?;
        self.reactor
            .websocket_send(self.fd, frame)
            .map_err(|error| map_browser_error(&error))
    }

    fn recv_message(
        &mut self,
        timeout: Duration,
    ) -> impl Future<Output = Result<(FrameHeader, Vec<u8>)>> + '_ {
        let receive = self.reactor.websocket_recv_async(self.fd);
        async move {
            let receive = receive.map_err(|error| map_browser_error(&error))?;
            let timer = WebTimer::new(timeout).map_err(|error| map_browser_error(&error))?;
            let bytes = ReceiveOrTimeout { receive, timer }
                .await
                .map_err(|error| map_browser_error(&error))?;
            decode_frame(&bytes)
        }
    }
}

struct ReceiveOrTimeout {
    receive: WebSocketReceive,
    timer: WebTimer,
}

struct OpenOrTimeout {
    open: WebSocketOpen,
    timer: WebTimer,
}

impl Future for OpenOrTimeout {
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Poll::Ready(result) = Pin::new(&mut this.open).poll(cx) {
            return Poll::Ready(result);
        }
        if let Poll::Ready(result) = Pin::new(&mut this.timer).poll(cx) {
            return Poll::Ready(match result {
                Ok(()) => Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "WebSocket OPEN deadline elapsed",
                )),
                Err(error) => Err(error),
            });
        }
        Poll::Pending
    }
}

impl Future for ReceiveOrTimeout {
    type Output = io::Result<Vec<u8>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Poll::Ready(result) = Pin::new(&mut this.receive).poll(cx) {
            return Poll::Ready(result);
        }
        if let Poll::Ready(result) = Pin::new(&mut this.timer).poll(cx) {
            return Poll::Ready(match result {
                Ok(()) => Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "WebSocket receive deadline elapsed",
                )),
                Err(error) => Err(error),
            });
        }
        Poll::Pending
    }
}

impl Drop for BrowserWebSocketTransport {
    fn drop(&mut self) {
        let result = self.reactor.websocket_close(self.fd);
        debug_assert!(
            result.is_ok(),
            "invariant: browser WebSocket remains registered until transport drop"
        );
    }
}

fn decode_frame(bytes: &[u8]) -> Result<(FrameHeader, Vec<u8>)> {
    if bytes.is_empty() {
        return Err(MetisError::protocol(
            ErrorCode::FrameTruncated,
            "Empty WebSocket message cannot contain a Metis frame",
        ));
    }
    check_wire_size(bytes)?;
    let mut wire = bytes;
    let frame = read_frame(&mut wire)?;
    if !wire.is_empty() {
        return Err(MetisError::protocol(
            ErrorCode::MalformedPayload,
            "Trailing bytes after WebSocket frame",
        ));
    }
    Ok(frame)
}

fn map_browser_error(error: &io::Error) -> MetisError {
    let code = match error.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::BrokenPipe | io::ErrorKind::UnexpectedEof => {
            ErrorCode::ConnectionClosed
        }
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => ErrorCode::Timeout,
        io::ErrorKind::InvalidData | io::ErrorKind::InvalidInput => ErrorCode::MalformedPayload,
        io::ErrorKind::OutOfMemory => ErrorCode::QueueFull,
        _ => ErrorCode::TransportBroken,
    };
    MetisError::transport(code, error.to_string())
}
