//! Correlated asynchronous IPC for browser-thread transports.

use crate::client::{
    CapabilityError, HandshakeError, decode_capability_response, decode_handshake_response,
    next_request, validate_response,
};
use crate::transport::AsyncIpcTransport;
use metis_core::capability::CapabilityToken;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{
    CapabilityCatalogPayload, HandshakeRequestPayload, MessageType, PROTOCOL_VERSION,
};
use std::collections::BTreeMap;
use std::time::Duration;

/// Maximum number of requests and completed responses retained by one client.
pub const MAX_PENDING_REQUESTS: usize = 16;

/// Correlation identifier assigned to one asynchronous request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct RequestId(u64);

impl RequestId {
    /// Returns the wire sequence identifier.
    #[must_use]
    pub const fn sequence(self) -> u64 {
        self.0
    }
}

/// Client state for a non-blocking transport.
pub struct AsyncIpcClient<T> {
    transport: T,
    next_seq: Option<u64>,
    active_token: Option<CapabilityToken>,
    request_timeout: Duration,
    pending: BTreeMap<u64, MessageType>,
    completed: BTreeMap<u64, (MessageType, Vec<u8>)>,
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
            pending: BTreeMap::new(),
            completed: BTreeMap::new(),
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

    /// Discovers the typed commands advertised by an authenticated host.
    ///
    /// # Errors
    /// Returns local transport, correlation, decoding, or protocol-version
    /// errors, or the peer's decoded rejection.
    pub async fn discover_capabilities(
        &mut self,
    ) -> std::result::Result<CapabilityCatalogPayload, CapabilityError> {
        let (kind, payload) = self.send_and_recv(MessageType::CapabilityReq, &[]).await?;
        decode_capability_response(kind, &payload)
    }

    /// Returns the current session capability, if acquired.
    pub const fn active_token(&self) -> Option<&CapabilityToken> {
        self.active_token.as_ref()
    }

    /// Replaces the capability with a caller-verified token.
    pub fn set_active_token(&mut self, token: CapabilityToken) {
        self.active_token = Some(token);
    }

    /// Sends a request and records its expected response type.
    ///
    /// Responses are collected by one receive consumer. Callers that need
    /// more than one request in flight send each request first, then await
    /// [`Self::recv_response_for`] for the identifiers they own. The bounded
    /// table matches the browser transport queue bound.
    ///
    /// # Errors
    /// Returns queue-capacity, request-type, sequence, or transport failures.
    pub fn send_request(&mut self, msg_type: MessageType, payload: &[u8]) -> Result<RequestId> {
        if self.pending.len() + self.completed.len() >= MAX_PENDING_REQUESTS {
            return Err(MetisError::transport(
                ErrorCode::QueueFull,
                "Async IPC request table is full",
            ));
        }
        let (expected, sequence) = next_request(&mut self.next_seq, msg_type)?;
        self.transport.send_message(msg_type, sequence, payload)?;
        self.pending.insert(sequence, expected);
        Ok(RequestId(sequence))
    }

    /// Cancels one outstanding request or retained response.
    ///
    /// Cancellation removes the identifier from the bounded correlation table.
    /// A later peer response for that identifier is therefore rejected as an
    /// unknown sequence instead of being delivered to a new request.
    ///
    /// # Errors
    /// Returns [`ErrorCode::SequenceMismatch`] when `request_id` is no longer
    /// pending or retained.
    pub fn cancel_request(&mut self, request_id: RequestId) -> Result<()> {
        let removed_pending = self.pending.remove(&request_id.0).is_some();
        let removed_completed = self.completed.remove(&request_id.0).is_some();
        if removed_pending || removed_completed {
            Ok(())
        } else {
            Err(Self::missing_request())
        }
    }

    /// Cancels every outstanding request and retained response.
    ///
    /// Returns the number of correlation entries removed. The active session
    /// capability is preserved, so a caller can submit a new request after
    /// cancelling an earlier operation.
    pub fn cancel_all_requests(&mut self) -> usize {
        let count = self.pending_request_count();
        self.pending.clear();
        self.completed.clear();
        count
    }

    /// Receives the next response from the single transport receive pump.
    ///
    /// The returned identifier lets a caller route an out-of-order response.
    /// Only one receive future may be polled at a time because the underlying
    /// browser WebSocket owns one callback waiter.
    ///
    /// # Errors
    /// Returns timeout, transport, sequence, or response-type failures.
    pub async fn recv_response(&mut self) -> Result<(RequestId, MessageType, Vec<u8>)> {
        if self.pending.is_empty() {
            return Err(MetisError::protocol(
                ErrorCode::SequenceMismatch,
                "Async IPC has no pending response",
            ));
        }
        let (header, response) = self.transport.recv_message(self.request_timeout).await?;
        let Some(expected) = self.pending.remove(&header.sequence_id) else {
            return Err(MetisError::protocol(
                ErrorCode::SequenceMismatch,
                "Response sequence does not match an outstanding request",
            ));
        };
        validate_response(expected, header.sequence_id, &header)?;
        Ok((RequestId(header.sequence_id), header.msg_type, response))
    }

    /// Receives the response for one request, retaining other responses.
    ///
    /// A single receive loop drains the transport and stores at most
    /// [`MAX_PENDING_REQUESTS`] completed responses, so out-of-order peer
    /// delivery cannot grow application memory without bound.
    ///
    /// # Errors
    /// Returns timeout, transport, sequence, or response-type failures.
    pub async fn recv_response_for(
        &mut self,
        request_id: RequestId,
    ) -> Result<(MessageType, Vec<u8>)> {
        if let Some(response) = self.completed.remove(&request_id.0) {
            return Ok(response);
        }
        if !self.pending.contains_key(&request_id.0) {
            return Err(Self::missing_request());
        }
        loop {
            let (received, message_type, payload) = self.recv_response().await?;
            if received == request_id {
                return Ok((message_type, payload));
            }
            if self
                .completed
                .insert(received.0, (message_type, payload))
                .is_some()
            {
                return Err(MetisError::protocol(
                    ErrorCode::SequenceMismatch,
                    "Async IPC received a duplicate response",
                ));
            }
        }
    }

    /// Returns the number of requests or completed responses retained.
    #[must_use]
    pub fn pending_request_count(&self) -> usize {
        self.pending.len() + self.completed.len()
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
        let request_id = self.send_request(msg_type, payload)?;
        self.recv_response_for(request_id).await
    }

    fn missing_request() -> MetisError {
        MetisError::protocol(
            ErrorCode::SequenceMismatch,
            "Async IPC request is not outstanding",
        )
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
    fn capability_discovery_uses_the_correlated_async_request() {
        let catalog = CapabilityCatalogPayload::new([
            MessageType::CapabilityReq,
            MessageType::ClinicalCalcReq,
        ])
        .expect("catalog");
        let response = frame(
            MessageType::CapabilityResp,
            1,
            &catalog.encode().expect("encoded catalog"),
        );
        let mut client =
            AsyncIpcClient::new(ScriptTransport::new([response]), Duration::from_secs(1))
                .expect("positive timeout");
        assert_eq!(
            poll_ready(client.discover_capabilities()).expect("catalog"),
            catalog
        );
        let wire = client.transport.sent.first().expect("request frame");
        let mut wire = wire.as_slice();
        let (header, payload) = crate::read_frame(&mut wire).expect("request");
        assert_eq!(header.msg_type, MessageType::CapabilityReq);
        assert!(payload.is_empty());
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
    fn response_type_is_checked_after_async_receive() {
        let response = frame(MessageType::ClinicalCalcResp, 1, b"response");
        let mut client =
            AsyncIpcClient::new(ScriptTransport::new([response]), Duration::from_secs(1))
                .expect("positive timeout");

        let error = poll_ready(client.send_and_recv(MessageType::HeartbeatReq, b"request"))
            .expect_err("wrong response type");
        assert_eq!(error.code, ErrorCode::UnexpectedMessageType);
    }

    #[test]
    fn out_of_order_responses_are_correlated_with_a_bounded_table() {
        let responses = [
            frame(MessageType::HeartbeatResp, 2, b"second"),
            frame(MessageType::HeartbeatResp, 1, b"first"),
        ];
        let mut client =
            AsyncIpcClient::new(ScriptTransport::new(responses), Duration::from_secs(1))
                .expect("positive timeout");

        let first = client
            .send_request(MessageType::HeartbeatReq, b"first")
            .expect("first request");
        let second = client
            .send_request(MessageType::HeartbeatReq, b"second")
            .expect("second request");
        assert_eq!(first.sequence(), 1);
        assert_eq!(second.sequence(), 2);

        assert_eq!(
            poll_ready(client.recv_response_for(first)).expect("first response"),
            (MessageType::HeartbeatResp, b"first".to_vec())
        );
        assert_eq!(
            poll_ready(client.recv_response_for(second)).expect("second response"),
            (MessageType::HeartbeatResp, b"second".to_vec())
        );
        assert_eq!(client.pending_request_count(), 0);
    }

    #[test]
    fn request_table_rejects_the_seventeenth_outstanding_request() {
        let mut client = AsyncIpcClient::new(ScriptTransport::new([]), Duration::from_secs(1))
            .expect("positive timeout");
        for _ in 0..MAX_PENDING_REQUESTS {
            client
                .send_request(MessageType::HeartbeatReq, b"request")
                .expect("bounded request capacity");
        }

        let error = client
            .send_request(MessageType::HeartbeatReq, b"overflow")
            .expect_err("seventeenth request must be rejected");
        assert_eq!(error.code, ErrorCode::QueueFull);
        assert_eq!(client.pending_request_count(), MAX_PENDING_REQUESTS);
    }

    #[test]
    fn unknown_response_sequence_is_rejected() {
        let response = frame(MessageType::HeartbeatResp, 9, b"unknown");
        let mut client =
            AsyncIpcClient::new(ScriptTransport::new([response]), Duration::from_secs(1))
                .expect("positive timeout");
        client
            .send_request(MessageType::HeartbeatReq, b"request")
            .expect("request");

        let error = poll_ready(client.recv_response()).expect_err("unknown sequence");
        assert_eq!(error.code, ErrorCode::SequenceMismatch);
        assert_eq!(client.pending_request_count(), 1);
    }

    #[test]
    fn cancellation_rejects_a_late_response_without_touching_newer_requests() {
        let response = frame(MessageType::HeartbeatResp, 1, b"late");
        let mut client =
            AsyncIpcClient::new(ScriptTransport::new([response]), Duration::from_secs(1))
                .expect("positive timeout");
        let cancelled = client
            .send_request(MessageType::HeartbeatReq, b"cancelled")
            .expect("cancelled request");
        client
            .send_request(MessageType::HeartbeatReq, b"active")
            .expect("active request");

        assert_eq!(client.cancel_request(cancelled), Ok(()));
        let error = poll_ready(client.recv_response()).expect_err("late response");
        assert_eq!(error.code, ErrorCode::SequenceMismatch);
        assert_eq!(client.pending_request_count(), 1);
        assert_eq!(
            client
                .cancel_request(cancelled)
                .expect_err("already cancelled")
                .code,
            ErrorCode::SequenceMismatch
        );
    }

    #[test]
    fn cancellation_clears_retained_responses_and_preserves_the_session() {
        let response = frame(MessageType::HeartbeatResp, 2, b"retained");
        let mut client =
            AsyncIpcClient::new(ScriptTransport::new([response]), Duration::from_secs(1))
                .expect("positive timeout");
        let active = token([9; 16]);
        client.set_active_token(active.clone());
        let first = client
            .send_request(MessageType::HeartbeatReq, b"first")
            .expect("first request");
        let second = client
            .send_request(MessageType::HeartbeatReq, b"second")
            .expect("second request");

        let error =
            poll_ready(client.recv_response_for(first)).expect_err("missing first response");
        assert_eq!(error.code, ErrorCode::ConnectionClosed);
        assert_eq!(client.pending_request_count(), 2);
        assert_eq!(client.cancel_request(second), Ok(()));
        assert_eq!(client.cancel_all_requests(), 1);
        assert_eq!(client.pending_request_count(), 0);
        assert_eq!(client.active_token(), Some(&active));
    }

    #[test]
    fn zero_timeout_is_rejected_before_a_request_can_start() {
        let Err(error) = AsyncIpcClient::new(ScriptTransport::new([]), Duration::ZERO) else {
            panic!("zero timeout must be rejected");
        };
        assert_eq!(error.code, ErrorCode::Timeout);
    }
}
