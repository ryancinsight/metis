//! The two error constructors the fragment codec raises.

use crate::error::{ErrorCode, MetisError};

pub(crate) fn malformed(message: &'static str) -> MetisError {
    MetisError::protocol(ErrorCode::MalformedPayload, message)
}

pub(crate) fn too_large(message: &'static str) -> MetisError {
    MetisError::protocol(ErrorCode::PayloadTooLarge, message)
}
