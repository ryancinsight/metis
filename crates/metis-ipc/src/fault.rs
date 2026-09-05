//! Fault injection at the encoded-wire boundary.
use crate::transport::{IpcTransport, check_wire_size};
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{FrameHeader, HEADER_SIZE};

/// Independent faults applied to outgoing encoded frames.
#[derive(Debug, Clone, Default)]
pub struct FaultConfig {
    /// Flips a checksum bit after encoding.
    pub corrupt_crc: bool,
    /// Removes the last wire byte without changing length or checksum.
    pub truncate_payload: bool,
    /// Permanently drops the wrapped endpoint on the next operation.
    pub drop_connection: bool,
}
/// Transport decorator that mutates actual wire bytes.
pub struct FaultInjectingTransport<T> {
    inner: Option<T>,
    /// Faults applied on the next operation.
    pub config: FaultConfig,
}
impl<T> FaultInjectingTransport<T> {
    /// Wraps an endpoint with the requested fault configuration.
    pub const fn new(inner: T, config: FaultConfig) -> Self {
        Self {
            inner: Some(inner),
            config,
        }
    }
    fn endpoint(&mut self) -> Result<&mut T> {
        if self.config.drop_connection {
            self.inner = None;
        }
        self.inner.as_mut().ok_or_else(|| {
            MetisError::transport(
                ErrorCode::ConnectionClosed,
                "Fault-injected connection closure",
            )
        })
    }
}
impl<T: IpcTransport> IpcTransport for FaultInjectingTransport<T> {
    fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
        check_wire_size(frame)?;
        let mut wire = frame.to_vec();
        if self.config.corrupt_crc
            && let Some(checksum_byte) = wire.get_mut(16)
        {
            *checksum_byte ^= 1;
        }
        if self.config.truncate_payload && wire.len() >= HEADER_SIZE {
            wire.truncate(wire.len() - 1);
        }
        self.endpoint()?.send_frame(&wire)
    }
    fn recv_message(&mut self) -> Result<(FrameHeader, Vec<u8>)> {
        self.endpoint()?.recv_message()
    }
}
