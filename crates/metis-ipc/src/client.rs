//! Correlated request/response client and session capability state.
use crate::transport::IpcTransport;
use metis_core::capability::CapabilityToken;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{
    CapabilityCatalogPayload, ErrorResponsePayload, FrameHeader, HandshakeRequestPayload,
    HandshakeResponsePayload, MessageType, PROTOCOL_VERSION, RemoteEventPayload,
};
use std::collections::VecDeque;

/// Maximum unsolicited events retained while a synchronous client awaits a response.
pub const MAX_QUEUED_EVENTS: usize = 16;

/// Failure to establish a session, preserving local faults and peer rejections.
///
/// Remote codes retain their wire value, including codes unknown to this client.
/// A malformed rejection is a local decoding failure, not a remote error.
///
/// # Examples
/// ```
/// use metis_core::protocol::{ErrorResponsePayload, MessageType};
/// use metis_ipc::client::HandshakeError;
/// use metis_ipc::{IpcClient, IpcTransport, MemoryTransport};
///
/// let (transport, mut peer) = MemoryTransport::pair();
/// let rejection = ErrorResponsePayload {
///     error_code: 0xffff,
///     message: "Session rejected".into(),
/// };
/// peer.send_message(MessageType::ErrorResp, 1, &rejection.encode()?)?;
/// let mut client = IpcClient::new(transport);
/// assert_eq!(client.handshake(1, [1; 16]), Err(HandshakeError::Remote(rejection)));
/// # Ok::<(), metis_core::error::MetisError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HandshakeError {
    /// Transport, correlation, decoding, or session validation fails locally.
    Local(MetisError),
    /// The correlated peer response rejects session initialization.
    Remote(ErrorResponsePayload),
}

impl From<MetisError> for HandshakeError {
    fn from(error: MetisError) -> Self {
        Self::Local(error)
    }
}

impl std::fmt::Display for HandshakeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local(error) => error.fmt(formatter),
            Self::Remote(error) => write!(
                formatter,
                "Peer rejected handshake [0x{:04X}]: {}",
                error.error_code, error.message
            ),
        }
    }
}

impl std::error::Error for HandshakeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // The standard error source API requires type erasure for diagnostics.
        match self {
            Self::Local(error) => Some(error),
            Self::Remote(_) => None,
        }
    }
}

/// Failure to discover a host catalog, preserving local faults and peer rejections.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CapabilityError {
    /// Transport, correlation, decoding, or version validation fails locally.
    Local(MetisError),
    /// The correlated peer response rejects capability discovery.
    Remote(ErrorResponsePayload),
}

impl From<MetisError> for CapabilityError {
    fn from(error: MetisError) -> Self {
        Self::Local(error)
    }
}

impl std::fmt::Display for CapabilityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local(error) => error.fmt(formatter),
            Self::Remote(error) => write!(
                formatter,
                "Peer rejected capability discovery [0x{:04X}]: {}",
                error.error_code, error.message
            ),
        }
    }
}

impl std::error::Error for CapabilityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Local(error) => Some(error),
            Self::Remote(_) => None,
        }
    }
}

/// Client with monotonically increasing sequence identifiers.
pub struct IpcClient<T> {
    transport: T,
    next_seq: Option<u64>,
    active_token: Option<CapabilityToken>,
    last_event_id: Option<u64>,
    events: VecDeque<RemoteEventPayload>,
}
impl<T: IpcTransport> IpcClient<T> {
    /// Starts a client without an authenticated session capability.
    pub const fn new(transport: T) -> Self {
        Self {
            transport,
            next_seq: Some(1),
            active_token: None,
            last_event_id: None,
            events: VecDeque::new(),
        }
    }
    /// Acquires a token whose principal and protocol version match the request.
    ///
    /// The transport establishes peer trust; principal matching does not prove
    /// server identity or verify the server's capability signature.
    /// # Errors
    /// Returns local transport, correlation, decoding, version, or principal
    /// errors, or the peer's decoded rejection without changing its wire code.
    pub fn handshake(
        &mut self,
        client_process_id: u32,
        principal_id: [u8; 16],
    ) -> Result<CapabilityToken, HandshakeError> {
        self.active_token = None;
        let req = HandshakeRequestPayload {
            client_version: PROTOCOL_VERSION,
            client_process_id,
            principal_id,
        };
        let (kind, payload) = self.send_and_recv(MessageType::HandshakeReq, &req.encode())?;
        let token = decode_handshake_response(kind, &payload, principal_id)?;
        self.active_token = Some(token.clone());
        Ok(token)
    }

    /// Discovers the typed commands advertised by an authenticated host.
    ///
    /// # Errors
    /// Returns local transport, correlation, decoding, or protocol-version
    /// errors, or the peer's decoded rejection.
    pub fn discover_capabilities(
        &mut self,
    ) -> std::result::Result<CapabilityCatalogPayload, CapabilityError> {
        let (kind, payload) = self.send_and_recv(MessageType::CapabilityReq, &[])?;
        decode_capability_response(kind, &payload)
    }

    /// Receives one unsolicited event from the message-oriented transport.
    ///
    /// Event identifiers must echo the envelope identifier and increase
    /// strictly for this client. Callers should invoke this method from the
    /// one receive owner for a connection; request responses are not consumed
    /// by an event-only call. Events observed while a request response is
    /// pending are returned from this bounded queue before reading the wire.
    ///
    /// # Errors
    /// Returns transport, decoding, message-type, or event-sequence errors.
    pub fn recv_event(&mut self) -> Result<RemoteEventPayload> {
        if let Some(event) = self.events.pop_front() {
            return Ok(event);
        }
        let (header, payload) = self.transport.recv_message()?;
        decode_event(&header, &payload, &mut self.last_event_id)
    }
    /// Returns the current session capability, if acquired.
    pub const fn active_token(&self) -> Option<&CapabilityToken> {
        self.active_token.as_ref()
    }
    /// Replaces the capability with a caller-verified token.
    pub fn set_active_token(&mut self, token: CapabilityToken) {
        self.active_token = Some(token);
    }
    /// Sends one request and validates its response type and sequence.
    ///
    /// A correlated structured error is returned for the caller to decode.
    /// # Errors
    /// Rejects non-request types, exhausted sequences, transport failures,
    /// unrelated sequence identifiers, and incompatible response types.
    /// Unsolicited events encountered before the correlated response are
    /// retained for [`Self::recv_event`] within the same bounded queue used by
    /// the event receiver.
    pub fn send_and_recv(
        &mut self,
        msg_type: MessageType,
        payload: &[u8],
    ) -> Result<(MessageType, Vec<u8>)> {
        let (expected, sequence) = next_request(&mut self.next_seq, msg_type)?;
        self.transport.send_message(msg_type, sequence, payload)?;
        loop {
            let (header, response) = self.transport.recv_message()?;
            if header.msg_type.is_event() {
                if self.events.len() >= MAX_QUEUED_EVENTS {
                    return Err(MetisError::transport(
                        ErrorCode::QueueFull,
                        "Synchronous IPC remote event queue is full",
                    ));
                }
                let event = decode_event(&header, &response, &mut self.last_event_id)?;
                self.events.push_back(event);
                continue;
            }
            validate_response(expected, sequence, &header)?;
            return Ok((header.msg_type, response));
        }
    }
}

pub(crate) fn next_request(
    next_seq: &mut Option<u64>,
    msg_type: MessageType,
) -> Result<(MessageType, u64)> {
    let expected = msg_type.response_type().ok_or_else(|| {
        MetisError::protocol(
            ErrorCode::UnexpectedMessageType,
            "Client can only send request message types",
        )
    })?;
    let sequence = next_seq.ok_or_else(|| {
        MetisError::protocol(
            ErrorCode::SequenceMismatch,
            "Client sequence space exhausted",
        )
    })?;
    *next_seq = sequence.checked_add(1);
    Ok((expected, sequence))
}

pub(crate) fn validate_response(
    expected: MessageType,
    sequence: u64,
    header: &FrameHeader,
) -> Result<()> {
    if header.sequence_id != sequence {
        return Err(MetisError::protocol(
            ErrorCode::SequenceMismatch,
            "Response sequence does not match request",
        ));
    }
    if header.msg_type != expected && header.msg_type != MessageType::ErrorResp {
        return Err(MetisError::protocol(
            ErrorCode::UnexpectedMessageType,
            "Response type does not match request",
        ));
    }
    Ok(())
}

pub(crate) fn decode_handshake_response(
    kind: MessageType,
    payload: &[u8],
    principal_id: [u8; 16],
) -> std::result::Result<CapabilityToken, HandshakeError> {
    if kind == MessageType::ErrorResp {
        return Err(HandshakeError::Remote(ErrorResponsePayload::decode(
            payload,
        )?));
    }
    let response = HandshakeResponsePayload::decode(payload)?;
    if response.server_version != PROTOCOL_VERSION {
        return Err(MetisError::protocol(
            ErrorCode::VersionMismatch,
            "Handshake server version mismatch",
        )
        .into());
    }
    if response.initial_token.principal_id != principal_id {
        return Err(MetisError::capability(
            ErrorCode::InvalidPrincipal,
            "Handshake token principal mismatch",
        )
        .into());
    }
    Ok(response.initial_token)
}

pub(crate) fn decode_capability_response(
    kind: MessageType,
    payload: &[u8],
) -> std::result::Result<CapabilityCatalogPayload, CapabilityError> {
    if kind == MessageType::ErrorResp {
        return Err(CapabilityError::Remote(ErrorResponsePayload::decode(
            payload,
        )?));
    }
    if kind != MessageType::CapabilityResp {
        return Err(MetisError::protocol(
            ErrorCode::UnexpectedMessageType,
            "Capability discovery response type does not match the request",
        )
        .into());
    }
    let catalog = CapabilityCatalogPayload::decode(payload)?;
    if catalog.protocol_version() != PROTOCOL_VERSION {
        return Err(MetisError::protocol(
            ErrorCode::VersionMismatch,
            "Capability catalog version differs from the wire contract",
        )
        .into());
    }
    Ok(catalog)
}

pub(crate) fn decode_event(
    header: &FrameHeader,
    payload: &[u8],
    last_event_id: &mut Option<u64>,
) -> Result<RemoteEventPayload> {
    if !header.msg_type.is_event() {
        return Err(MetisError::protocol(
            ErrorCode::UnexpectedMessageType,
            "Event receiver requires an unsolicited event message",
        ));
    }
    let event = RemoteEventPayload::decode(payload)?;
    if event.protocol_version() != PROTOCOL_VERSION {
        return Err(MetisError::protocol(
            ErrorCode::VersionMismatch,
            "Remote event version differs from the wire contract",
        ));
    }
    let event_id = event.event_id().get();
    if header.sequence_id != event_id {
        return Err(MetisError::protocol(
            ErrorCode::SequenceMismatch,
            "Remote event header and envelope identifiers differ",
        ));
    }
    if last_event_id.is_some_and(|last| event_id <= last) {
        return Err(MetisError::protocol(
            ErrorCode::ReplayDetected,
            "Remote event identifiers must increase strictly",
        ));
    }
    *last_event_id = Some(event_id);
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryTransport;

    #[test]
    fn exhausted_sequence_never_wraps_to_zero() {
        let (transport, mut peer) = MemoryTransport::pair();
        let mut client = IpcClient::new(transport);
        client.next_seq = Some(u64::MAX);
        peer.send_message(MessageType::HeartbeatResp, u64::MAX, b"last")
            .expect("response");
        assert_eq!(
            client
                .send_and_recv(MessageType::HeartbeatReq, b"last")
                .expect("last sequence"),
            (MessageType::HeartbeatResp, b"last".to_vec())
        );
        assert_eq!(
            peer.recv_message().expect("request").0.sequence_id,
            u64::MAX
        );
        assert_eq!(
            client
                .send_and_recv(MessageType::HeartbeatReq, b"overflow")
                .expect_err("exhausted")
                .code,
            ErrorCode::SequenceMismatch
        );
    }

    #[test]
    fn capability_discovery_decodes_the_versioned_catalog() {
        let catalog = CapabilityCatalogPayload::new([
            MessageType::CapabilityReq,
            MessageType::ClinicalCalcReq,
        ])
        .expect("catalog");
        let decoded = decode_capability_response(
            MessageType::CapabilityResp,
            &catalog.encode().expect("encoded catalog"),
        )
        .expect("catalog response");
        assert_eq!(decoded, catalog);
    }

    #[test]
    fn capability_discovery_rejects_an_unadvertised_host_operation() {
        let error = decode_capability_response(
            MessageType::ErrorResp,
            &ErrorResponsePayload {
                error_code: ErrorCode::UnexpectedMessageType as u16,
                message: ErrorCode::UnexpectedMessageType.as_str().to_owned(),
            }
            .encode()
            .expect("error payload"),
        )
        .expect_err("unsupported catalog");
        assert_eq!(
            error,
            CapabilityError::Remote(ErrorResponsePayload {
                error_code: ErrorCode::UnexpectedMessageType as u16,
                message: ErrorCode::UnexpectedMessageType.as_str().to_owned(),
            })
        );
    }

    #[test]
    fn capability_discovery_rejects_a_catalog_version_mismatch() {
        let catalog = CapabilityCatalogPayload::new([MessageType::HeartbeatReq]).expect("catalog");
        let mut payload = catalog.encode().expect("encoded catalog");
        payload[..2].copy_from_slice(&0x0200_u16.to_be_bytes());
        let error = decode_capability_response(MessageType::CapabilityResp, &payload)
            .expect_err("version mismatch");
        assert!(matches!(
            error,
            CapabilityError::Local(error) if error.code == ErrorCode::VersionMismatch
        ));
    }

    #[test]
    fn receives_a_typed_remote_event_and_rejects_replayed_ids() {
        let (transport, mut peer) = MemoryTransport::pair();
        let event = RemoteEventPayload::new(4, "test.value", [0x12, 0x34]).expect("event");
        peer.send_message(
            MessageType::TelemetryStreamEvent,
            event.event_id().get(),
            &event.encode().expect("event payload"),
        )
        .expect("event frame");
        let mut client = IpcClient::new(transport);
        assert_eq!(client.recv_event().expect("event"), event);

        peer.send_message(
            MessageType::TelemetryStreamEvent,
            event.event_id().get(),
            &event.encode().expect("event payload"),
        )
        .expect("replayed event frame");
        assert_eq!(
            client.recv_event().expect_err("replayed event").code,
            ErrorCode::ReplayDetected
        );
    }

    #[test]
    fn rejects_a_remote_event_version_mismatch() {
        let (transport, mut peer) = MemoryTransport::pair();
        let event = RemoteEventPayload::new(1, "test.value", [0x12, 0x34]).expect("event");
        let mut payload = event.encode().expect("event payload");
        payload[..2].copy_from_slice(&0x0200_u16.to_be_bytes());
        peer.send_message(
            MessageType::TelemetryStreamEvent,
            event.event_id().get(),
            &payload,
        )
        .expect("event frame");
        let mut client = IpcClient::new(transport);
        assert_eq!(
            client.recv_event().expect_err("version mismatch").code,
            ErrorCode::VersionMismatch
        );
    }

    #[test]
    fn retains_an_event_seen_before_its_correlated_response() {
        let (transport, mut peer) = MemoryTransport::pair();
        let event = RemoteEventPayload::new(1, "test.value", [0x12, 0x34]).expect("event");
        peer.send_message(
            MessageType::TelemetryStreamEvent,
            event.event_id().get(),
            &event.encode().expect("event payload"),
        )
        .expect("event frame");
        peer.send_message(MessageType::HeartbeatResp, 1, b"response")
            .expect("response frame");

        let mut client = IpcClient::new(transport);
        assert_eq!(
            client
                .send_and_recv(MessageType::HeartbeatReq, b"request")
                .expect("correlated response"),
            (MessageType::HeartbeatResp, b"response".to_vec())
        );
        assert_eq!(client.recv_event().expect("queued event"), event);
    }
}
