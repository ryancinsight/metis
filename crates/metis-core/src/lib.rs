#![deny(missing_docs)]
#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

pub mod capability;
pub mod crypto;
pub mod error;
pub mod protocol;

pub use capability::{CapabilityScope, CapabilityToken, VerifiedCapability};
pub use crypto::{Sha256, constant_time_eq_32, crc32, hmac_sha256, sha256};
pub use error::{ErrorCode, MetisError, Result};
pub use protocol::{
    FrameHeader, HEADER_SIZE, MAX_PAYLOAD_SIZE, MessageType, PROTOCOL_MAGIC, PROTOCOL_VERSION,
    build_frame,
};
