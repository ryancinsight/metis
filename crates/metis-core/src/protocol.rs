//! Versioned binary IPC wire contracts.
//!
//! Frames contain magic (4 bytes), version (2), type (2), sequence (8),
//! payload CRC32 (4), payload length (4), then payload bytes. Integers and
//! IEEE-754 binary64 clinical fields use big-endian byte order. CRC detects
//! accidental corruption; it does not authenticate a peer or a payload.
mod command;
mod payload;
mod wire;
pub use command::{CapabilityCatalogPayload, CommandDescriptor, MAX_COMMANDS, SUPPORTED_COMMANDS};
pub use payload::{
    ClinicalCalcRequestPayload, ClinicalCalcResponsePayload, ErrorResponsePayload,
    HandshakeRequestPayload, HandshakeResponsePayload,
};
pub use wire::{
    FrameHeader, HEADER_SIZE, MAX_PAYLOAD_SIZE, MessageType, PROTOCOL_MAGIC, PROTOCOL_VERSION,
    build_frame,
};
