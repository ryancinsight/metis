//! Correlated asynchronous IPC for browser-thread transports.

use crate::client::{HandshakeError, decode_handshake_response, next_request, validate_response};
use crate::transport::AsyncIpcTransport;
use metis_core::capability::CapabilityToken;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{HandshakeRequestPayload, MessageType, PROTOCOL_VERSION};
use std::time::Duration;

/// Client state for a non-blocking transport.
pub struct AsyncIpcClient<T> {
    transport: T,
    next_seq: Option<u64>,
    active_token: Option<CapabilityToken>,
    request_timeout: Duration,
}

impl<T: AsyncIpcTransport> AsyncIpcClient<T> {
    /// Starts a client with a finite timeout for every receive operation.
    ///
    /// # Errors
    /// Returns [`ErrorCode::Timeout`] when `request_timeout` is zero.
    pub fn new(transport: T, request_timeout: Duration) -> Result<Self> {
        if request_timeout.is_zero() {
            return Err(MetisError::transport(
                ErrorCode::Timeout,
                "Async IPC request timeout must be non-zero",
            ));
        }
        Ok(Self {
            transport,
            next_seq: Some(1),
            active_token: None,
            request_timeout,
        })
    }

    /// Acquires a token without blocking the browser event thread.
    ///
    /// The transport establishes peer trust; principal matching does not prove
    /// server identity or verify the server's capability signature.
    ///
    /// # Errors
    /// Returns transport, correlation, decoding, version, or principal errors,
    /// or the peer's decoded rejection without changing its wire code.
    pub async fn handshake(
        &mut self,
        client_process_id: u32,
        principal_id: [u8; 16],
    ) -> std::result::Result<CapabilityToken, HandshakeError> {
        self.active_token = None;
        let request = HandshakeRequestPayload {
            client_version: PROTOCOL_VERSION,
            client_process_id,
            principal_id,
        };
        let (kind, payload) = self
            .send_and_recv(MessageType::HandshakeReq, &request.encode())
            .await?;
        let token = decode_handshake_response(kind, &payload, principal_id)?;
        self.active_token = Some(token.clone());
        Ok(token)
    }

    /// Returns the current session capability, if acquired.
    pub const fn active_token(&self) -> Option<&CapabilityToken> {
        self.active_token.as_ref()
    }

    /// Replaces the capability with a caller-verified token.
    pub fn set_active_token(&mut self, token: CapabilityToken) {
        self.active_token = Some(token);
    }

    /// Sends one request and awaits its correlated response.
    ///
    /// The sequence is consumed before sending, so a transport failure cannot
    /// cause an uncertain operation to be retried with the same identifier.
    ///
    /// # Errors
    /// Rejects non-request types, exhausted sequences, transport failures,
    /// unrelated sequence identifiers, and incompatible response types.
    pub async fn send_and_recv(
        &mut self,
        msg_type: MessageType,
        payload: &[u8],
    ) -> Result<(MessageType, Vec<u8>)> {
        let (expected, sequence) = next_request(&mut self.next_seq, msg_type)?;
        self.transport.send_message(msg_type, sequence, payload)?;
        let (header, response) = self.transport.recv_message(self.request_timeout).await?;
        validate_response(expected, sequence, &header)?;
        Ok((header.msg_type, response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsyncIpcTransport;
    use metis_core::capability::{CapabilityScope, CapabilityToken};
    use metis_core::error::{ErrorCode, MetisError};
    use metis_core::protocol::{FrameHeader, HandshakeResponsePayload, build_frame};
    use std::collections::VecDeque;
    use std::future::{Future, ready};
    use std::task::{Context, Poll, Waker};

    struct ScriptTransport {
        responses: VecDeque<Result<(FrameHeader, Vec<u8>)>>,
        sent: Vec<Vec<u8>>,
    }

    impl ScriptTransport {
        fn new(responses: impl IntoIterator<Item = Result<(FrameHeader, Vec<u8>)>>) -> Self {
            Self {
                responses: responses.into_iter().collect(),
                sent: Vec::new(),
            }
        }
    }

    impl AsyncIpcTransport for ScriptTransport {
        fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
            self.sent.push(frame.to_vec());
            Ok(())
        }

        fn recv_message(
            &mut self,
            _timeout: Duration,
        ) -> impl Future<Output = Result<(FrameHeader, Vec<u8>)>> + '_ {
            ready(self.responses.pop_front().unwrap_or_else(|| {
                Err(MetisError::transport(
                    ErrorCode::ConnectionClosed,
                    "Script transport has no response",
                ))
            }))
        }
    }

    fn frame(
        message_type: MessageType,
        sequence: u64,
        payload: &[u8],
    ) -> Result<(FrameHeader, Vec<u8>)> {
        let wire = build_frame(message_type, sequence, payload)?;
        let mut bytes = wire.as_slice();
        crate::read_frame(&mut bytes)
    }

    fn token(principal_id: [u8; 16]) -> CapabilityToken {
        CapabilityToken::issue(
            7,
            principal_id,
            CapabilityScope::UI_RENDER,
            10,
            20,
            30,
            b"test key",
        )
    }

    fn poll_ready<F: Future>(future: F) -> F::Output {
        let mut future = Box::pin(future);
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("script transport future must be ready"),
        }
    }

    #[test]
    fn handshake_round_trip_preserves_token_and_wire_request() {
        let principal = [7; 16];
        let response = HandshakeResponsePayload {
            server_version: PROTOCOL_VERSION,
            initial_token: token(principal),
        };
        let response = frame(MessageType::HandshakeResp, 1, &response.encode());
        let mut client =
            AsyncIpcClient::new(ScriptTransport::new([response]), Duration::from_secs(1))
                .expect("positive timeout");

        let acquired = poll_ready(client.handshake(42, principal)).expect("handshake");
        assert_eq!(acquired, *client.active_token().expect("active token"));

        assert_eq!(client.transport.sent.len(), 1);
        let wire = client.transport.sent.first().expect("one request");
        let mut wire = wire.as_slice();
        let (header, payload) = crate::read_frame(&mut wire).expect("request frame");
        assert_eq!(header.msg_type, MessageType::HandshakeReq);
        assert_eq!(header.sequence_id, 1);
        let request = HandshakeRequestPayload::decode(&payload).expect("request payload");
        assert_eq!(request.client_process_id, 42);
        assert_eq!(request.principal_id, principal);
    }

    #[test]
    fn handshake_rejection_keeps_remote_code_and_clears_session() {
        let rejection = metis_core::protocol::ErrorResponsePayload {
            error_code: 0xffff,
            message: "Session rejected".to_owned(),
        };
        let response = frame(
            MessageType::ErrorResp,
            1,
            &rejection.encode().expect("payload"),
        );
        let mut client =
            AsyncIpcClient::new(ScriptTransport::new([response]), Duration::from_secs(1))
                .expect("positive timeout");
        client.set_active_token(token([1; 16]));

        assert_eq!(
            poll_ready(client.handshake(42, [2; 16])),
            Err(HandshakeError::Remote(rejection))
        );
        assert_eq!(client.active_token(), None);
    }

    #[test]
    fn response_correlation_is_checked_after_async_receive() {
        let response = frame(MessageType::HeartbeatResp, 2, b"response");
        let mut client =
            AsyncIpcClient::new(ScriptTransport::new([response]), Duration::from_secs(1))
                .expect("positive timeout");

        let error = poll_ready(client.send_and_recv(MessageType::HeartbeatReq, b"request"))
            .expect_err("wrong sequence");
        assert_eq!(error.code, ErrorCode::SequenceMismatch);
    }

    #[test]
    fn zero_timeout_is_rejected_before_a_request_can_start() {
        let Err(error) = AsyncIpcClient::new(ScriptTransport::new([]), Duration::ZERO) else {
            panic!("zero timeout must be rejected");
        };
        assert_eq!(error.code, ErrorCode::Timeout);
    }
}
