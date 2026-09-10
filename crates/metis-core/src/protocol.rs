//! Versioned binary IPC wire contracts.
//!
//! Frames contain magic (4 bytes), version (2), type (2), sequence (8),
//! payload CRC32 (4), payload length (4), then payload bytes. Integers and
//! IEEE-754 binary64 clinical fields use big-endian byte order. CRC detects
//! accidental corruption; it does not authenticate a peer or a payload.
mod command;
mod diagnostic;
mod event;
mod fragment;
mod payload;
mod plugin;
mod target;
mod wire;
pub use command::{CapabilityCatalogPayload, CommandDescriptor, MAX_COMMANDS, SUPPORTED_COMMANDS};
pub use event::{EventCodec, EventId, MAX_EVENT_NAME_BYTES, RemoteEventPayload};
pub use fragment::{
    FragmentAction, FragmentPatch, FragmentPatchSet, MAX_FRAGMENT_ACTION_BYTES,
    MAX_FRAGMENT_ATTRIBUTE_BYTES, MAX_FRAGMENT_BODY_BYTES, MAX_FRAGMENT_PATCHES,
    MAX_FRAGMENT_TARGET_BYTES, MAX_FRAGMENT_VALUE_BYTES,
};
pub use payload::{
    ClinicalCalcRequestPayload, ClinicalCalcResponsePayload, ErrorResponsePayload,
    HandshakeRequestPayload, HandshakeResponsePayload, PluginInvocationPayload,
    PluginInvocationResponsePayload,
};
pub use plugin::{
    MAX_PLUGIN_NAME_BYTES, MAX_PLUGIN_OPERATION_NAME_BYTES, MAX_PLUGIN_OPERATIONS, MAX_PLUGINS,
    Plugin, PluginDescriptor, PluginOperation, PluginRegistry,
};
pub use target::{
    MAX_TARGET_CAPABILITIES, TargetCapability, TargetCapabilityPayload, TargetPlatform,
};
pub use wire::{
    FrameHeader, HEADER_SIZE, MAX_PAYLOAD_SIZE, MessageType, PROTOCOL_MAGIC, PROTOCOL_VERSION,
    build_frame,
};
