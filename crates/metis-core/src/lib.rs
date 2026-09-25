#![deny(missing_docs)]
#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

pub mod capability;
pub mod command_line;
pub mod crypto;
pub mod deep_link;
pub mod error;
pub mod host;
pub mod input;
pub mod protocol;
pub mod route;
mod uri_path;
pub mod window_state;

pub use capability::{CapabilityGrantSpec, CapabilityScope, CapabilityToken, VerifiedCapability};
pub use crypto::crc32;
pub use error::{ErrorCode, MetisError, RedactedDiagnostic, Result};
pub use host::{
    HostContext, HostOrigin, HostPolicy, HostSessionId, VerifiedHostCapability, WindowId,
};
pub use protocol::{
    CapabilityCatalogPayload, CommandDescriptor, EventCodec, EventId, FragmentAction,
    FragmentPatch, FragmentPatchSet, FrameHeader, HEADER_SIZE, MAX_COMMANDS, MAX_EVENT_NAME_BYTES,
    MAX_FRAGMENT_ACTION_BYTES, MAX_FRAGMENT_ATTRIBUTE_BYTES, MAX_FRAGMENT_BODY_BYTES,
    MAX_FRAGMENT_PATCHES, MAX_FRAGMENT_TARGET_BYTES, MAX_FRAGMENT_VALUE_BYTES, MAX_PAYLOAD_SIZE,
    MAX_PLUGIN_NAME_BYTES, MAX_PLUGIN_OPERATION_NAME_BYTES, MAX_PLUGIN_OPERATIONS, MAX_PLUGINS,
    MAX_TARGET_CAPABILITIES, MessageType, PROTOCOL_MAGIC, PROTOCOL_VERSION, Plugin,
    PluginDescriptor, PluginInvocationPayload, PluginInvocationResponsePayload, PluginOperation,
    PluginRegistry, RemoteEventPayload, SUPPORTED_COMMANDS, TargetCapability,
    TargetCapabilityPayload, TargetPlatform, build_frame,
};
