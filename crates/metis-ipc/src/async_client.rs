//! Correlated asynchronous IPC for browser-thread transports.

#[cfg(test)]
mod tests;

use crate::client::{
    CapabilityError, HandshakeError, PluginInvocationError, TargetCapabilityError,
    decode_capability_response, decode_event, decode_handshake_response, decode_plugin_response,
    decode_target_capability_response, next_request, validate_response,
};
use crate::transport::AsyncIpcTransport;
use metis_core::capability::CapabilityToken;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{
    CapabilityCatalogPayload, HandshakeRequestPayload, MessageType, PROTOCOL_VERSION,
    PluginInvocationPayload, PluginInvocationResponsePayload, RemoteEventPayload,
    TargetCapabilityPayload,
};
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::time::Duration;

/// Maximum number of requests and completed responses retained by one client.
pub const MAX_PENDING_REQUESTS: usize = 16;

pub use crate::client::MAX_QUEUED_EVENTS;

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
    events: VecDeque<RemoteEventPayload>,
    last_event_id: Option<u64>,
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
            events: VecDeque::with_capacity(MAX_QUEUED_EVENTS),
            last_event_id: None,
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

    /// Discovers the connected host target and its implemented surfaces.
    ///
    /// # Errors
    /// Returns local transport, correlation, decoding, or protocol-version
    /// errors, or the peer's decoded rejection.
    pub async fn discover_target_capabilities(
        &mut self,
    ) -> std::result::Result<TargetCapabilityPayload, TargetCapabilityError> {
        let (kind, payload) = self
            .send_and_recv(MessageType::TargetCapabilityReq, &[])
            .await?;
        decode_target_capability_response(kind, &payload)
    }

    /// Invokes one authenticated plugin operation without blocking the caller.
    ///
    /// The payload carries the capability token and plugin-owned body codec;
    /// this client validates only the outer response envelope.
    ///
    /// # Errors
    /// Returns local transport, correlation, decoding, or response-type errors,
    /// or the peer's decoded rejection.
    pub async fn invoke_plugin(
        &mut self,
        request: &PluginInvocationPayload,
    ) -> std::result::Result<PluginInvocationResponsePayload, PluginInvocationError> {
        let encoded = request.encode()?;
        let (kind, payload) = self
            .send_and_recv(MessageType::PluginInvokeReq, &encoded)
            .await?;
        decode_plugin_response(kind, &payload)
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
    /// unknown sequence instead of being delivered to a new request. The
    /// receive pump remains usable for other outstanding requests after the
    /// caller handles that typed rejection.
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
        loop {
            let (header, response) = self.transport.recv_message(self.request_timeout).await?;
            if header.msg_type.is_event() {
                if self.events.len() >= MAX_QUEUED_EVENTS {
                    return Err(MetisError::transport(
                        ErrorCode::QueueFull,
                        "Async IPC remote event queue is full",
                    ));
                }
                let event = decode_event(&header, &response, &mut self.last_event_id)?;
                self.events.push_back(event);
                continue;
            }
            let Some(expected) = self.pending.remove(&header.sequence_id) else {
                return Err(MetisError::protocol(
                    ErrorCode::SequenceMismatch,
                    "Response sequence does not match an outstanding request",
                ));
            };
            validate_response(expected, header.sequence_id, &header)?;
            return Ok((RequestId(header.sequence_id), header.msg_type, response));
        }
    }

    /// Receives one remote event, retaining any correlated responses encountered.
    ///
    /// A single receive owner must call this method for a connection. Responses
    /// for outstanding requests are moved into the bounded completed table so
    /// a later [`Self::recv_response_for`] call can retrieve them.
    ///
    /// # Errors
    /// Returns timeout, transport, event decoding, sequence, or response-type
    /// failures.
    pub async fn recv_event(&mut self) -> Result<RemoteEventPayload> {
        if let Some(event) = self.events.pop_front() {
            return Ok(event);
        }
        loop {
            let (header, payload) = self.transport.recv_message(self.request_timeout).await?;
            if header.msg_type.is_event() {
                return decode_event(&header, &payload, &mut self.last_event_id);
            }
            let Some(expected) = self.pending.remove(&header.sequence_id) else {
                return Err(MetisError::protocol(
                    ErrorCode::SequenceMismatch,
                    "Response sequence does not match an outstanding request",
                ));
            };
            validate_response(expected, header.sequence_id, &header)?;
            if self
                .completed
                .insert(header.sequence_id, (header.msg_type, payload))
                .is_some()
            {
                return Err(MetisError::protocol(
                    ErrorCode::SequenceMismatch,
                    "Async IPC received a duplicate response",
                ));
            }
        }
    }

    /// Removes the oldest event received by the connection pump.
    #[must_use]
    pub fn poll_event(&mut self) -> Option<RemoteEventPayload> {
        self.events.pop_front()
    }

    /// Returns the number of queued unsolicited events.
    #[must_use]
    pub fn queued_event_count(&self) -> usize {
        self.events.len()
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
