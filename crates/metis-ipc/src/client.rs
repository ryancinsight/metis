//! Correlated request/response client and session capability state.

mod error;
#[cfg(test)]
mod tests;

pub use error::{CapabilityError, HandshakeError, PluginInvocationError, TargetCapabilityError};

use crate::transport::IpcTransport;
use metis_core::capability::CapabilityToken;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{
    CapabilityCatalogPayload, ErrorResponsePayload, FrameHeader, HandshakeRequestPayload,
    HandshakeResponsePayload, MessageType, PROTOCOL_VERSION, PluginInvocationPayload,
    PluginInvocationResponsePayload, RemoteEventPayload, TargetCapabilityPayload,
};
use std::collections::VecDeque;

/// Maximum unsolicited events retained while a synchronous client awaits a response.
pub const MAX_QUEUED_EVENTS: usize = 16;

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

    /// Discovers the connected host target and its implemented surfaces.
    ///
    /// # Errors
    /// Returns local transport, correlation, decoding, or protocol-version
    /// errors, or the peer's decoded rejection.
    pub fn discover_target_capabilities(
        &mut self,
    ) -> std::result::Result<TargetCapabilityPayload, TargetCapabilityError> {
        let (kind, payload) = self.send_and_recv(MessageType::TargetCapabilityReq, &[])?;
        decode_target_capability_response(kind, &payload)
    }

    /// Invokes one authenticated plugin operation and decodes its bounded body.
    ///
    /// The payload carries the capability token and the plugin-owned body
    /// codec; this client only validates the outer response envelope.
    ///
    /// # Errors
    /// Returns local transport, correlation, decoding, or response-type errors,
    /// or the peer's decoded rejection.
    pub fn invoke_plugin(
        &mut self,
        request: &PluginInvocationPayload,
    ) -> std::result::Result<PluginInvocationResponsePayload, PluginInvocationError> {
        let encoded = request.encode()?;
        let (kind, payload) = self.send_and_recv(MessageType::PluginInvokeReq, &encoded)?;
        decode_plugin_response(kind, &payload)
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

pub(crate) fn decode_target_capability_response(
    kind: MessageType,
    payload: &[u8],
) -> std::result::Result<TargetCapabilityPayload, TargetCapabilityError> {
    if kind == MessageType::ErrorResp {
        return Err(TargetCapabilityError::Remote(ErrorResponsePayload::decode(
            payload,
        )?));
    }
    if kind != MessageType::TargetCapabilityResp {
        return Err(MetisError::protocol(
            ErrorCode::UnexpectedMessageType,
            "Target capability discovery response type does not match the request",
        )
        .into());
    }
    let descriptor = TargetCapabilityPayload::decode(payload)?;
    if descriptor.protocol_version() != PROTOCOL_VERSION {
        return Err(MetisError::protocol(
            ErrorCode::VersionMismatch,
            "Target capability descriptor version differs from the wire contract",
        )
        .into());
    }
    Ok(descriptor)
}

pub(crate) fn decode_plugin_response(
    kind: MessageType,
    payload: &[u8],
) -> std::result::Result<PluginInvocationResponsePayload, PluginInvocationError> {
    if kind == MessageType::ErrorResp {
        return Err(PluginInvocationError::Remote(ErrorResponsePayload::decode(
            payload,
        )?));
    }
    if kind != MessageType::PluginInvokeResp {
        return Err(MetisError::protocol(
            ErrorCode::UnexpectedMessageType,
            "Plugin invocation response type does not match the request",
        )
        .into());
    }
    Ok(PluginInvocationResponsePayload::decode(payload)?)
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
