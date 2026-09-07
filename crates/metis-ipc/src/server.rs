//! Request dispatch with per-connection replay rejection.
use crate::transport::IpcTransport;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{FrameHeader, MessageType, RemoteEventPayload};

/// Identity available after a frame header has been decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestIdentity {
    /// Operation named by the peer's frame header.
    pub message_type: MessageType,
    /// Correlation identifier named by that header.
    pub sequence: u64,
}
impl From<&FrameHeader> for RequestIdentity {
    fn from(header: &FrameHeader) -> Self {
        Self {
            message_type: header.msg_type,
            sequence: header.sequence_id,
        }
    }
}

/// Stage of a failure outside normal application response processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureContext {
    /// Transport or framing rejected input before delivering a validated header.
    Receive,
    /// Request kind or sequence was rejected before application dispatch.
    Request(RequestIdentity),
    /// Application dispatch failed without producing a response.
    Handler(RequestIdentity),
    /// Response validation or transmission failed after application dispatch.
    Response(RequestIdentity),
}

/// Response produced after the request and handler contracts have passed.
#[derive(Debug)]
pub(crate) struct DispatchedResponse {
    /// Identity of the request that produced the response.
    pub identity: RequestIdentity,
    /// Wire message type to send to the peer.
    pub message_type: MessageType,
    /// Encoded response payload.
    pub payload: Vec<u8>,
}

/// Request handler invoked only after frame and sequence validation.
pub trait IpcHandler {
    /// Computes the request's response.
    /// # Errors
    /// Returns application failures that prevent generating a response.
    fn handle_request(
        &mut self,
        header: &FrameHeader,
        payload: &[u8],
    ) -> Result<(MessageType, Vec<u8>)>;

    /// Records a failure which cannot be represented by a normal response.
    ///
    /// Called once per server failure, excluding clean EOF. An ordinary
    /// application `ErrorResp` does not call this method. A response write
    /// failure follows a completed handler call and is a distinct delivery
    /// event, not evidence that the application operation was undone.
    ///
    /// # Errors
    /// Returns audit or diagnostic sink failures; these stop the server.
    fn handle_failure(&mut self, context: FailureContext, error: ErrorCode) -> Result<()>;
}
/// Server tracking the greatest accepted sequence on a connection.
pub struct IpcServer<T> {
    transport: T,
    last_sequence: u64,
    last_event_id: Option<u64>,
}
impl<T: IpcTransport> IpcServer<T> {
    /// Starts a server accepting positive, strictly increasing request sequences.
    pub const fn new(transport: T) -> Self {
        Self {
            transport,
            last_sequence: 0,
            last_event_id: None,
        }
    }
    /// Processes one request, or returns false for a clean peer close.
    ///
    /// Accepted sequences are consumed before dispatch, even when the handler
    /// fails, preventing retry of a request that may already have side effects.
    /// # Errors
    /// Returns malformed-frame, replay, transport, or handler failures.
    pub fn step<H: IpcHandler>(&mut self, handler: &mut H) -> Result<bool> {
        let (header, payload) = match self.transport.recv_message() {
            Ok(message) => message,
            Err(error) if error.code == ErrorCode::ConnectionClosed => return Ok(false),
            Err(error) => {
                handler.handle_failure(FailureContext::Receive, error.code)?;
                return Err(error);
            }
        };
        let dispatched = dispatch_request(&mut self.last_sequence, handler, header, &payload)?;
        if let Err(error) = self.transport.send_message(
            dispatched.message_type,
            dispatched.identity.sequence,
            &dispatched.payload,
        ) {
            handler.handle_failure(FailureContext::Response(dispatched.identity), error.code)?;
            return Err(error);
        }
        Ok(true)
    }

    /// Sends one unsolicited event with a strictly increasing identifier.
    ///
    /// Event identifiers use a sequence independent from request correlation.
    /// The identifier is consumed before transmission so an uncertain write
    /// cannot be retried as the same event.
    ///
    /// # Errors
    /// Returns payload, replay, or transport failures.
    pub fn send_event(&mut self, event: &RemoteEventPayload) -> Result<()> {
        let event_id = event.event_id().get();
        if self.last_event_id.is_some_and(|last| event_id <= last) {
            return Err(MetisError::protocol(
                ErrorCode::ReplayDetected,
                "Remote event identifiers must increase strictly",
            ));
        }
        let payload = event.encode()?;
        self.last_event_id = Some(event_id);
        self.transport
            .send_message(MessageType::TelemetryStreamEvent, event_id, &payload)
    }
}

/// Validate and dispatch one decoded request for any message-oriented server.
///
/// The sequence is consumed before the handler runs. Both synchronous pipe
/// sessions and asynchronous WebSocket sessions therefore share identical
/// replay, response-type, and failure-context semantics.
///
/// # Errors
/// Returns protocol or handler failures after recording the corresponding
/// failure context through `handler`.
pub(crate) fn dispatch_request<H: IpcHandler>(
    last_sequence: &mut u64,
    handler: &mut H,
    header: FrameHeader,
    payload: &[u8],
) -> Result<DispatchedResponse> {
    let identity = RequestIdentity::from(&header);
    let Some(expected) = header.msg_type.response_type() else {
        let error = MetisError::protocol(
            ErrorCode::UnexpectedMessageType,
            "Server accepts request message types only",
        );
        handler.handle_failure(FailureContext::Request(identity), error.code)?;
        return Err(error);
    };
    if header.sequence_id <= *last_sequence {
        let error = MetisError::protocol(
            ErrorCode::ReplayDetected,
            "Request sequence must increase strictly",
        );
        handler.handle_failure(FailureContext::Request(identity), error.code)?;
        return Err(error);
    }
    *last_sequence = header.sequence_id;
    let (message_type, payload) = match handler.handle_request(&header, payload) {
        Ok(response) => response,
        Err(error) => {
            handler.handle_failure(FailureContext::Handler(identity), error.code)?;
            return Err(error);
        }
    };
    if message_type != expected && message_type != MessageType::ErrorResp {
        let error = MetisError::protocol(
            ErrorCode::UnexpectedMessageType,
            "Handler response type does not match request",
        );
        handler.handle_failure(FailureContext::Response(identity), error.code)?;
        return Err(error);
    }
    Ok(DispatchedResponse {
        identity,
        message_type,
        payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::IpcClient;
    use crate::transport::MemoryTransport;

    #[test]
    fn sync_server_sends_a_bounded_remote_event_once() {
        let (client_transport, server_transport) = MemoryTransport::pair();
        let mut server = IpcServer::new(server_transport);
        let event = RemoteEventPayload::new(1, "test.value", [0x12, 0x34]).expect("event");
        server.send_event(&event).expect("event send");

        let mut client = IpcClient::new(client_transport);
        assert_eq!(client.recv_event().expect("event receive"), event);
        assert_eq!(
            server.send_event(&event).expect_err("replay").code,
            ErrorCode::ReplayDetected
        );
    }
}
