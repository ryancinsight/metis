//! Native Metis framing over the authenticated Moirai QUIC provider.

use crate::frame::split_frame;
use crate::transport::{IpcTransport, check_wire_size};
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{FrameHeader, HEADER_SIZE, MAX_PAYLOAD_SIZE};
use moirai_quic::{QuicConfig, QuicError, QuicStream};
use std::time::Duration;

/// ALPN for the Metis protocol-major-1 binary session mapping.
pub const METIS_QUIC_ALPN: &[u8] = b"metis-ipc/1";

/// Maximum complete Metis wire frame, including its validated header.
#[expect(
    clippy::cast_possible_truncation,
    reason = "The compile-time wire-domain assertion below proves this usize-to-u32 conversion"
)]
pub const MAX_QUIC_FRAME_BYTES: u32 = (HEADER_SIZE + MAX_PAYLOAD_SIZE) as u32;
const _: () = assert!(HEADER_SIZE + MAX_PAYLOAD_SIZE <= u32::MAX as usize);

/// Construct full-domain Metis frame, stream and connection admission limits.
///
/// Handshake bytes bound queued TLS handshakes independently of Metis frames.
///
/// # Errors
/// Returns zero/overflowing resource limits or invalid finite deadlines.
pub fn quic_config(
    streams: u32,
    connections: u32,
    handshake_bytes: u32,
    operation_timeout: Duration,
    idle_timeout: Duration,
) -> std::result::Result<QuicConfig, QuicError> {
    QuicConfig::new(
        METIS_QUIC_ALPN,
        MAX_QUIC_FRAME_BYTES,
        streams,
        connections,
        handshake_bytes,
        operation_timeout,
        idle_timeout,
    )
}

/// Bounded synchronous IPC client transport over a native QUIC stream.
///
/// The existing [`crate::IpcClient`] performs correlation, replay and capability
/// decoding. Moirai parks the calling native thread during bounded I/O; call
/// this client from native/blocking host work, not an event-loop callback.
/// Native servers use [`crate::AsyncIpcServer`] and await stream backpressure.
pub struct QuicIpcTransport {
    stream: QuicStream,
}

impl QuicIpcTransport {
    /// Admit a stream negotiated for Metis with its full wire-size budget.
    ///
    /// # Errors
    /// Returns wrong ALPN or an insufficient frame budget.
    pub fn new(stream: QuicStream) -> Result<Self> {
        validate_stream(&stream)?;
        Ok(Self { stream })
    }

    /// Queue FIN after the final request, preserving the response direction.
    ///
    /// Keep the parent connection alive until the peer consumes session output.
    ///
    /// # Errors
    /// Returns a closed/cancelled stream error.
    pub fn finish(&mut self) -> Result<()> {
        self.stream.finish().map_err(|error| quic_error(&error))
    }

    /// Finish a consumed session and wait for the peer's clean FIN.
    ///
    /// Call after consuming all expected replies/events. Unexpected trailing
    /// frames are validated and rejected; they are not silently discarded.
    /// Both halves use finite transport deadlines. Keep the parent connection
    /// alive until this method returns.
    ///
    /// # Errors
    /// Returns trailing-message, frame, peer-reset, deadline or transport errors.
    pub fn close(mut self) -> Result<()> {
        self.finish()?;
        match moirai_executor::block_on(self.stream.recv_message())
            .map_err(|error| quic_error(&error))?
        {
            None => moirai_executor::block_on(self.stream.finish_and_wait())
                .map_err(|error| quic_error(&error)),
            Some(frame) => {
                let (header, _) = split_frame(frame)?;
                Err(MetisError::protocol(
                    ErrorCode::UnexpectedMessageType,
                    format!("QUIC peer sends {:?} after session FIN", header.msg_type),
                ))
            }
        }
    }
}

impl IpcTransport for QuicIpcTransport {
    fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
        check_wire_size(frame)?;
        moirai_executor::block_on(self.stream.send_message(frame))
            .map_err(|error| quic_error(&error))
    }

    fn recv_message(&mut self) -> Result<(FrameHeader, Vec<u8>)> {
        let frame = moirai_executor::block_on(self.stream.recv_message())
            .map_err(|error| quic_error(&error))?
            .ok_or_else(|| {
                MetisError::transport(
                    ErrorCode::ConnectionClosed,
                    "QUIC peer finished the session",
                )
            })?;
        split_frame(frame)
    }
}

pub(crate) fn validate_stream(stream: &QuicStream) -> Result<()> {
    if stream.alpn() != METIS_QUIC_ALPN {
        return Err(MetisError::protocol(
            ErrorCode::VersionMismatch,
            "QUIC stream did not negotiate metis-ipc/1",
        ));
    }
    if stream.max_frame_bytes() < MAX_QUIC_FRAME_BYTES {
        return Err(MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "QUIC stream cannot admit the complete Metis wire domain",
        ));
    }
    Ok(())
}

pub(crate) fn quic_error(error: &QuicError) -> MetisError {
    let code = match error {
        QuicError::MessageTooLarge { .. } => ErrorCode::PayloadTooLarge,
        QuicError::InvalidConfig(_) => ErrorCode::MalformedPayload,
        QuicError::ConnectionCapacity | QuicError::StreamCapacity => ErrorCode::QueueFull,
        _ => match error.io_kind() {
            std::io::ErrorKind::TimedOut => ErrorCode::Timeout,
            std::io::ErrorKind::UnexpectedEof => ErrorCode::FrameTruncated,
            std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::BrokenPipe => ErrorCode::ConnectionClosed,
            _ => ErrorCode::TransportBroken,
        },
    };
    if matches!(
        code,
        ErrorCode::PayloadTooLarge | ErrorCode::MalformedPayload
    ) {
        MetisError::protocol(code, error.to_string())
    } else {
        MetisError::transport(code, error.to_string())
    }
}
