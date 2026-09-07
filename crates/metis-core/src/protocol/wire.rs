//! Fixed frame header and bounded frame construction.
use crate::crypto::crc32;
use crate::error::{ErrorCode, MetisError, Result};
/// Wire magic: ASCII METI.
pub const PROTOCOL_MAGIC: [u8; 4] = *b"METI";
/// Protocol major 1, minor 0.
pub const PROTOCOL_VERSION: u16 = 0x0100;
/// Header size in bytes.
pub const HEADER_SIZE: usize = 24;
/// Maximum encoded payload size in bytes.
pub const MAX_PAYLOAD_SIZE: usize = 65_536;
/// Wire message identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[repr(u16)]
pub enum MessageType {
    /// Initial session request.
    HandshakeReq = 1,
    /// Initial session response.
    HandshakeResp = 2,
    /// Liveness request.
    HeartbeatReq = 3,
    /// Liveness response.
    HeartbeatResp = 4,
    /// Capability catalog request.
    CapabilityReq = 5,
    /// Capability catalog response.
    CapabilityResp = 6,
    /// Host target capability request.
    TargetCapabilityReq = 7,
    /// Host target capability response.
    TargetCapabilityResp = 8,
    /// Clinical calculation request.
    ClinicalCalcReq = 0x10,
    /// Clinical calculation response.
    ClinicalCalcResp = 0x11,
    /// Audit query request.
    AuditQueryReq = 0x20,
    /// Audit query response.
    AuditQueryResp = 0x21,
    /// Unsolicited telemetry event.
    TelemetryStreamEvent = 0x30,
    /// Remote plugin invocation request.
    PluginInvokeReq = 0x40,
    /// Remote plugin invocation response.
    PluginInvokeResp = 0x41,
    /// Structured failure response.
    ErrorResp = 0xff,
}
impl MessageType {
    /// Resolves a known wire identifier.
    #[must_use]
    pub const fn from_u16(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::HandshakeReq),
            2 => Some(Self::HandshakeResp),
            3 => Some(Self::HeartbeatReq),
            4 => Some(Self::HeartbeatResp),
            5 => Some(Self::CapabilityReq),
            6 => Some(Self::CapabilityResp),
            7 => Some(Self::TargetCapabilityReq),
            8 => Some(Self::TargetCapabilityResp),
            0x10 => Some(Self::ClinicalCalcReq),
            0x11 => Some(Self::ClinicalCalcResp),
            0x20 => Some(Self::AuditQueryReq),
            0x21 => Some(Self::AuditQueryResp),
            0x30 => Some(Self::TelemetryStreamEvent),
            0x40 => Some(Self::PluginInvokeReq),
            0x41 => Some(Self::PluginInvokeResp),
            0xff => Some(Self::ErrorResp),
            _ => None,
        }
    }
    /// Returns the successful response type for a request, if this is a request.
    #[must_use]
    pub const fn response_type(self) -> Option<Self> {
        match self {
            Self::HandshakeReq => Some(Self::HandshakeResp),
            Self::HeartbeatReq => Some(Self::HeartbeatResp),
            Self::CapabilityReq => Some(Self::CapabilityResp),
            Self::TargetCapabilityReq => Some(Self::TargetCapabilityResp),
            Self::ClinicalCalcReq => Some(Self::ClinicalCalcResp),
            Self::AuditQueryReq => Some(Self::AuditQueryResp),
            Self::PluginInvokeReq => Some(Self::PluginInvokeResp),
            _ => None,
        }
    }

    /// Returns whether this identifier selects an unsolicited event.
    #[must_use]
    pub const fn is_event(self) -> bool {
        matches!(self, Self::TelemetryStreamEvent)
    }
}
/// Decoded fixed-size frame header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// Message contract selecting the payload decoder.
    pub msg_type: MessageType,
    /// Request identifier echoed by its response.
    pub sequence_id: u64,
    /// CRC32 of the encoded payload.
    pub payload_crc32: u32,
    /// Encoded payload length in bytes.
    pub payload_len: u32,
}
impl FrameHeader {
    /// Encodes the header using the current wire version.
    #[must_use]
    pub fn encode(&self) -> [u8; HEADER_SIZE] {
        let mut buf = [0; HEADER_SIZE];
        buf[..4].copy_from_slice(&PROTOCOL_MAGIC);
        buf[4..6].copy_from_slice(&PROTOCOL_VERSION.to_be_bytes());
        buf[6..8].copy_from_slice(&(self.msg_type as u16).to_be_bytes());
        buf[8..16].copy_from_slice(&self.sequence_id.to_be_bytes());
        buf[16..20].copy_from_slice(&self.payload_crc32.to_be_bytes());
        buf[20..].copy_from_slice(&self.payload_len.to_be_bytes());
        buf
    }
    /// Decodes the header and validates magic, version, type, and resource bound.
    /// # Errors
    /// Returns the error corresponding to the first invalid header field.
    pub fn decode(buf: &[u8; HEADER_SIZE]) -> Result<Self> {
        if buf[..4] != PROTOCOL_MAGIC {
            return Err(MetisError::protocol(
                ErrorCode::MagicMismatch,
                "Invalid wire magic",
            ));
        }
        if u16::from_be_bytes([buf[4], buf[5]]) != PROTOCOL_VERSION {
            return Err(MetisError::protocol(
                ErrorCode::VersionMismatch,
                "Unsupported wire version",
            ));
        }
        let msg_type =
            MessageType::from_u16(u16::from_be_bytes([buf[6], buf[7]])).ok_or_else(|| {
                MetisError::protocol(
                    ErrorCode::UnexpectedMessageType,
                    "Unknown wire message identifier",
                )
            })?;
        let mut remaining = &buf[8..];
        let sequence_id = u64::from_be_bytes(take(&mut remaining)?);
        let payload_crc32 = u32::from_be_bytes(take(&mut remaining)?);
        let payload_len = u32::from_be_bytes(take(&mut remaining)?);
        check_length(payload_len as usize)?;
        Ok(Self {
            msg_type,
            sequence_id,
            payload_crc32,
            payload_len,
        })
    }
}
/// Builds a frame after validating the payload resource bound.
/// # Errors
/// Returns `PayloadTooLarge` if the payload exceeds 65,536 bytes.
pub fn build_frame(msg_type: MessageType, sequence_id: u64, payload: &[u8]) -> Result<Vec<u8>> {
    check_length(payload.len())?;
    let payload_len = u32::try_from(payload.len()).map_err(|_| {
        MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Payload length cannot fit wire field",
        )
    })?;
    let header = FrameHeader {
        msg_type,
        sequence_id,
        payload_crc32: crc32(payload),
        payload_len,
    };
    let mut frame = Vec::with_capacity(HEADER_SIZE + payload.len());
    frame.extend_from_slice(&header.encode());
    frame.extend_from_slice(payload);
    Ok(frame)
}
pub(super) fn check_length(length: usize) -> Result<()> {
    if length > MAX_PAYLOAD_SIZE {
        return Err(MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Payload exceeds wire resource bound",
        ));
    }
    Ok(())
}
pub(super) fn malformed(message: &str) -> MetisError {
    MetisError::protocol(ErrorCode::MalformedPayload, message)
}
pub(super) fn take<const N: usize>(remaining: &mut &[u8]) -> Result<[u8; N]> {
    let (bytes, rest) = remaining
        .split_first_chunk::<N>()
        .ok_or_else(|| malformed("Truncated payload field"))?;
    *remaining = rest;
    Ok(*bytes)
}
pub(super) fn finish(remaining: &[u8]) -> Result<()> {
    if !remaining.is_empty() {
        return Err(malformed("Trailing payload bytes"));
    }
    Ok(())
}
