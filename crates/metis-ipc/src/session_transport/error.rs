//! WebSocket message-error classification at the native adapter boundary.

use metis_core::error::{ErrorCode, MetisError};
use std::io;

pub(super) fn websocket_error(error: &io::Error) -> MetisError {
    let message = error.to_string();
    let code = match error.kind() {
        io::ErrorKind::InvalidData if message.contains("exceeds configured byte bound") => {
            ErrorCode::PayloadTooLarge
        }
        io::ErrorKind::InvalidData | io::ErrorKind::InvalidInput => ErrorCode::MalformedPayload,
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => ErrorCode::Timeout,
        io::ErrorKind::UnexpectedEof => ErrorCode::FrameTruncated,
        io::ErrorKind::BrokenPipe => ErrorCode::ConnectionClosed,
        _ => ErrorCode::TransportBroken,
    };
    if matches!(
        code,
        ErrorCode::MalformedPayload | ErrorCode::PayloadTooLarge
    ) {
        MetisError::protocol(code, message)
    } else {
        MetisError::transport(code, message)
    }
}
