//! Stream and bounded in-memory transports sharing the same wire decoder.

use crate::frame::{read_frame, write_wire};
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{FrameHeader, HEADER_SIZE, MAX_PAYLOAD_SIZE, MessageType, build_frame};
use std::future::Future;
use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::time::Duration;

/// Bidirectional transport with a wire boundary for corruption testing.
pub trait IpcTransport: Send {
    /// Sends encoded wire bytes. Each call represents one complete frame.
    ///
    /// This boundary accepts malformed frames for protocol fault injection.
    /// # Errors
    /// Returns capacity, size, deadline, or transport failures.
    fn send_frame(&mut self, frame: &[u8]) -> Result<()>;
    /// Encodes and sends a bounded message.
    /// # Errors
    /// Returns payload-size or transport failures.
    fn send_message(
        &mut self,
        msg_type: MessageType,
        sequence_id: u64,
        payload: &[u8],
    ) -> Result<()> {
        self.send_frame(&build_frame(msg_type, sequence_id, payload)?)
    }
    /// Receives and validates a frame.
    /// # Errors
    /// Returns deadline, transport, or wire-validation failures.
    fn recv_message(&mut self) -> Result<(FrameHeader, Vec<u8>)>;
}

/// Event-driven transport for browser and other non-blocking hosts.
///
/// The send side is immediate: implementations either hand bytes to a
/// non-blocking host API or return a bounded backpressure error. Receipt is a
/// future so a browser event thread never waits on a synchronous read. The
/// trait deliberately has no `Send` bound because browser futures and Web API
/// handles remain on the JavaScript event-loop thread.
pub trait AsyncIpcTransport {
    /// Sends encoded wire bytes immediately.
    ///
    /// # Errors
    /// Returns capacity, size, or transport failures.
    fn send_frame(&mut self, frame: &[u8]) -> Result<()>;

    /// Encodes and sends a bounded message.
    ///
    /// # Errors
    /// Returns payload-size, backpressure, or transport failures.
    fn send_message(
        &mut self,
        msg_type: MessageType,
        sequence_id: u64,
        payload: &[u8],
    ) -> Result<()> {
        self.send_frame(&build_frame(msg_type, sequence_id, payload)?)
    }

    /// Receives and validates one frame within the supplied finite deadline.
    ///
    /// # Errors
    /// The future returns timeout, transport, or wire-validation failures.
    fn recv_message(
        &mut self,
        timeout: Duration,
    ) -> impl Future<Output = Result<(FrameHeader, Vec<u8>)>> + '_;
}

/// IPC over caller-owned streams.
///
/// Callers must set read/write deadlines on the underlying streams before
/// construction. Generic `Read`/`Write` cannot interrupt blocking OS pipe I/O.
pub struct StreamTransport<R, W> {
    reader: R,
    writer: W,
}
impl<R, W> StreamTransport<R, W> {
    /// Takes ownership of streams with caller-configured I/O deadlines.
    pub const fn new(reader: R, writer: W) -> Self {
        Self { reader, writer }
    }
}
impl<R: Read + Send, W: Write + Send> IpcTransport for StreamTransport<R, W> {
    fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
        check_wire_size(frame)?;
        write_wire(&mut self.writer, frame)
    }
    fn recv_message(&mut self) -> Result<(FrameHeader, Vec<u8>)> {
        read_frame(&mut self.reader)
    }
}

/// Bounded message transport for deterministic integration tests.
///
/// Each direction stores at most 16 frames of 65,560 bytes. Sending never
/// blocks: a full queue reports backpressure. Receiving has a finite deadline.
pub struct MemoryTransport {
    sender: SyncSender<Vec<u8>>,
    receiver: Receiver<Vec<u8>>,
    timeout: Duration,
}
impl MemoryTransport {
    /// Connects two endpoints with a five-second receive deadline.
    #[must_use]
    pub fn pair() -> (Self, Self) {
        Self::pair_with_timeout(Duration::from_secs(5))
    }
    /// Connects endpoints with the supplied finite receive deadline.
    #[must_use]
    pub fn pair_with_timeout(timeout: Duration) -> (Self, Self) {
        // Sixteen outstanding requests bound each direction to approximately
        // one MiB; request/response clients normally have one outstanding frame.
        let (tx_a, rx_b) = sync_channel(16);
        let (tx_b, rx_a) = sync_channel(16);
        (
            Self {
                sender: tx_a,
                receiver: rx_a,
                timeout,
            },
            Self {
                sender: tx_b,
                receiver: rx_b,
                timeout,
            },
        )
    }
}
impl IpcTransport for MemoryTransport {
    fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
        check_wire_size(frame)?;
        self.sender
            .try_send(frame.to_vec())
            .map_err(|error| match error {
                TrySendError::Full(_) => {
                    MetisError::transport(ErrorCode::QueueFull, "Memory transport queue is full")
                }
                TrySendError::Disconnected(_) => MetisError::transport(
                    ErrorCode::ConnectionClosed,
                    "Memory transport peer closed",
                ),
            })
    }
    fn recv_message(&mut self) -> Result<(FrameHeader, Vec<u8>)> {
        let bytes = self
            .receiver
            .recv_timeout(self.timeout)
            .map_err(|error| match error {
                RecvTimeoutError::Timeout => MetisError::transport(
                    ErrorCode::Timeout,
                    "Memory transport receive deadline elapsed",
                ),
                RecvTimeoutError::Disconnected => MetisError::transport(
                    ErrorCode::ConnectionClosed,
                    "Memory transport peer closed",
                ),
            })?;
        let mut wire = bytes.as_slice();
        if wire.is_empty() {
            return Err(MetisError::protocol(
                ErrorCode::FrameTruncated,
                "Empty wire frame received",
            ));
        }
        let message = read_frame(&mut wire)?;
        if !wire.is_empty() {
            return Err(MetisError::protocol(
                ErrorCode::MalformedPayload,
                "Trailing bytes after memory transport frame",
            ));
        }
        Ok(message)
    }
}
pub(crate) fn check_wire_size(frame: &[u8]) -> Result<()> {
    if frame.len() > HEADER_SIZE + MAX_PAYLOAD_SIZE {
        return Err(MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Wire frame exceeds transport capacity",
        ));
    }
    Ok(())
}
