//! Browser session failure mapping at the host boundary.

use metis_frontend::FormState;
use metis_ipc::client::HandshakeError;

pub(crate) fn connect_failure_state(error: HandshakeError) -> FormState {
    match error {
        HandshakeError::Local(error) => FormState::Disconnected(error),
        error => FormState::SessionFailed(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use metis_core::error::{ErrorCode, MetisError};
    use metis_core::protocol::ErrorResponsePayload;

    #[test]
    fn remote_handshake_code_reaches_the_session_state_unchanged() {
        let rejection = ErrorResponsePayload {
            error_code: 0xffff,
            message: "test peer rejection".to_owned(),
        };
        let state = connect_failure_state(HandshakeError::Remote(rejection.clone()));
        assert_eq!(
            state,
            FormState::SessionFailed(HandshakeError::Remote(rejection))
        );
    }

    #[test]
    fn local_handshake_failure_remains_a_disconnected_state() {
        let error = MetisError::transport(ErrorCode::ConnectionClosed, "test transport");
        assert!(matches!(
            connect_failure_state(HandshakeError::Local(error)),
            FormState::Disconnected(error) if error.code == ErrorCode::ConnectionClosed
        ));
    }
}
