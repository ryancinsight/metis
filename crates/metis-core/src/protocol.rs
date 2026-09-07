//! Versioned binary IPC wire contracts.
//!
//! Frames contain magic (4 bytes), version (2), type (2), sequence (8),
//! payload CRC32 (4), payload length (4), then payload bytes. Integers and
//! IEEE-754 binary64 clinical fields use big-endian byte order. CRC detects
//! accidental corruption; it does not authenticate a peer or a payload.
mod command;
mod event;
mod payload;
mod plugin;
mod wire;
pub use command::{CapabilityCatalogPayload, CommandDescriptor, MAX_COMMANDS, SUPPORTED_COMMANDS};
pub use event::{EventCodec, EventId, MAX_EVENT_NAME_BYTES, RemoteEventPayload};
pub use payload::{
    ClinicalCalcRequestPayload, ClinicalCalcResponsePayload, ErrorResponsePayload,
    HandshakeRequestPayload, HandshakeResponsePayload,
};
pub use plugin::{
    MAX_PLUGIN_NAME_BYTES, MAX_PLUGIN_OPERATION_NAME_BYTES, MAX_PLUGIN_OPERATIONS, MAX_PLUGINS,
    Plugin, PluginDescriptor, PluginOperation, PluginRegistry,
};
pub use wire::{
    FrameHeader, HEADER_SIZE, MAX_PAYLOAD_SIZE, MessageType, PROTOCOL_MAGIC, PROTOCOL_VERSION,
    build_frame,
};
