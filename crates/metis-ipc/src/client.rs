//! Correlated request/response client and session capability state.
use crate::transport::IpcTransport;
use metis_core::capability::CapabilityToken;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{
    ErrorResponsePayload, FrameHeader, HandshakeRequestPayload, HandshakeResponsePayload,
    MessageType, PROTOCOL_VERSION,
};

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
    /// The correlated peer response rejects the handshake.
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

/// Client with monotonically increasing sequence identifiers.
pub struct IpcClient<T> {
    transport: T,
    next_seq: Option<u64>,
    active_token: Option<CapabilityToken>,
}
impl<T: IpcTransport> IpcClient<T> {
    /// Starts a client without an authenticated session capability.
    pub const fn new(transport: T) -> Self {
        Self {
            transport,
            next_seq: Some(1),
            active_token: None,
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
    pub fn send_and_recv(
        &mut self,
        msg_type: MessageType,
        payload: &[u8],
    ) -> Result<(MessageType, Vec<u8>)> {
        let (expected, sequence) = next_request(&mut self.next_seq, msg_type)?;
        self.transport.send_message(msg_type, sequence, payload)?;
        let (header, response) = self.transport.recv_message()?;
        validate_response(expected, sequence, &header)?;
        Ok((header.msg_type, response))
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
}
