#![deny(missing_docs)]
#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

pub mod capability;
pub mod crypto;
pub mod error;
pub mod host;
pub mod protocol;

pub use capability::{CapabilityGrantSpec, CapabilityScope, CapabilityToken, VerifiedCapability};
pub use crypto::crc32;
pub use error::{ErrorCode, MetisError, Result};
pub use host::{
    HostContext, HostOrigin, HostPolicy, HostSessionId, VerifiedHostCapability, WindowId,
};
pub use protocol::{
    CapabilityCatalogPayload, CommandDescriptor, EventCodec, EventId, FrameHeader, HEADER_SIZE,
    MAX_COMMANDS, MAX_EVENT_NAME_BYTES, MAX_PAYLOAD_SIZE, MessageType, PROTOCOL_MAGIC,
    PROTOCOL_VERSION, RemoteEventPayload, SUPPORTED_COMMANDS, build_frame,
};
