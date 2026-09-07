#![deny(missing_docs)]
#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

pub mod capability;
pub mod crypto;
pub mod error;
pub mod protocol;

pub use capability::{CapabilityScope, CapabilityToken, VerifiedCapability};
pub use crypto::crc32;
pub use error::{ErrorCode, MetisError, Result};
pub use protocol::{
    FrameHeader, HEADER_SIZE, MAX_PAYLOAD_SIZE, MessageType, PROTOCOL_MAGIC, PROTOCOL_VERSION,
    build_frame,
};
