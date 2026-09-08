//! Redacted formatting for diagnostics received across the wire.

use super::payload::ErrorResponsePayload;
use std::fmt;

impl fmt::Debug for ErrorResponsePayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ErrorResponsePayload")
            .field("error_code", &self.error_code)
            .field("message", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::ErrorResponsePayload;

    #[test]
    fn remote_debug_diagnostic_omits_message_text() {
        let error = ErrorResponsePayload {
            error_code: 0x4001,
            message: "authorization token and patient path".to_owned(),
        };
        let debug = format!("{error:?}");
        assert!(debug.contains("16385"));
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains("authorization"));
        assert!(!debug.contains("patient"));
    }
}
