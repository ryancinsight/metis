//! Poll-driven correlation, capability, and event behaviour.

use super::*;
use crate::AsyncIpcTransport;
use metis_core::capability::{CapabilityScope, CapabilityToken};
use metis_core::error::{ErrorCode, MetisError};
use metis_core::protocol::{FrameHeader, HandshakeResponsePayload, build_frame};
use metis_core::protocol::{TargetCapability, TargetPlatform};
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
    let mut client = AsyncIpcClient::new(ScriptTransport::new([response]), Duration::from_secs(1))
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
    let mut client = AsyncIpcClient::new(ScriptTransport::new([response]), Duration::from_secs(1))
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
    let catalog =
        CapabilityCatalogPayload::new([MessageType::CapabilityReq, MessageType::ClinicalCalcReq])
            .expect("catalog");
    let response = frame(
        MessageType::CapabilityResp,
        1,
        &catalog.encode().expect("encoded catalog"),
    );
    let mut client = AsyncIpcClient::new(ScriptTransport::new([response]), Duration::from_secs(1))
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
fn target_capability_discovery_uses_the_correlated_async_request() {
    let descriptor = TargetCapabilityPayload::new(
        TargetPlatform::Linux,
        [
            TargetCapability::NativeProcess,
            TargetCapability::BrowserWebSocket,
        ],
    )
    .expect("descriptor");
    let response = frame(
        MessageType::TargetCapabilityResp,
        1,
        &descriptor.encode().expect("encoded descriptor"),
    );
    let mut client = AsyncIpcClient::new(ScriptTransport::new([response]), Duration::from_secs(1))
        .expect("positive timeout");
    assert_eq!(
        poll_ready(client.discover_target_capabilities()).expect("target descriptor"),
        descriptor
    );
    let wire = client.transport.sent.first().expect("request frame");
    let mut wire = wire.as_slice();
    let (header, payload) = crate::read_frame(&mut wire).expect("request");
    assert_eq!(header.msg_type, MessageType::TargetCapabilityReq);
    assert_eq!(header.sequence_id, 1);
    assert!(payload.is_empty());
}

#[test]
fn plugin_invocation_decodes_the_correlated_async_response() {
    let token = token([1; 16]);
    let request = PluginInvocationPayload::new(token, "viewer", "open", [1, 4, 9])
        .expect("plugin invocation");
    let response = PluginInvocationResponsePayload::new([2, 5, 10])
        .expect("plugin response")
        .encode()
        .expect("encoded response");
    let response = frame(MessageType::PluginInvokeResp, 1, &response);
    let mut client = AsyncIpcClient::new(ScriptTransport::new([response]), Duration::from_secs(1))
        .expect("positive timeout");

    let response = poll_ready(client.invoke_plugin(&request)).expect("plugin response");
    assert_eq!(response.body(), [2, 5, 10]);
    let wire = client.transport.sent.first().expect("request frame");
    let mut wire = wire.as_slice();
    let (header, payload) = crate::read_frame(&mut wire).expect("request");
    assert_eq!(header.msg_type, MessageType::PluginInvokeReq);
    assert_eq!(PluginInvocationPayload::decode(&payload), Ok(request));
}

#[test]
fn response_correlation_is_checked_after_async_receive() {
    let response = frame(MessageType::HeartbeatResp, 2, b"response");
    let mut client = AsyncIpcClient::new(ScriptTransport::new([response]), Duration::from_secs(1))
        .expect("positive timeout");

    let error = poll_ready(client.send_and_recv(MessageType::HeartbeatReq, b"request"))
        .expect_err("wrong sequence");
    assert_eq!(error.code, ErrorCode::SequenceMismatch);
}

#[test]
fn response_type_is_checked_after_async_receive() {
    let response = frame(MessageType::ClinicalCalcResp, 1, b"response");
    let mut client = AsyncIpcClient::new(ScriptTransport::new([response]), Duration::from_secs(1))
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
    let mut client = AsyncIpcClient::new(ScriptTransport::new(responses), Duration::from_secs(1))
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
fn response_pump_retains_remote_events_while_correlating_a_request() {
    let event = RemoteEventPayload::new(9, "test.value", [0x12, 0x34]).expect("event");
    let responses = [
        frame(
            MessageType::TelemetryStreamEvent,
            event.event_id().get(),
            &event.encode().expect("event payload"),
        ),
        frame(MessageType::HeartbeatResp, 1, b"response"),
    ];
    let mut client = AsyncIpcClient::new(ScriptTransport::new(responses), Duration::from_secs(1))
        .expect("positive timeout");
    let request = client
        .send_request(MessageType::HeartbeatReq, b"request")
        .expect("request");

    assert_eq!(
        poll_ready(client.recv_response_for(request)).expect("response"),
        (MessageType::HeartbeatResp, b"response".to_vec())
    );
    assert_eq!(client.queued_event_count(), 1);
    assert_eq!(client.poll_event(), Some(event));
}

#[test]
fn event_pump_retains_correlated_responses_for_later_consumers() {
    let event = RemoteEventPayload::new(9, "test.value", [0x56, 0x78]).expect("event");
    let responses = [
        frame(MessageType::HeartbeatResp, 1, b"response"),
        frame(
            MessageType::TelemetryStreamEvent,
            event.event_id().get(),
            &event.encode().expect("event payload"),
        ),
    ];
    let mut client = AsyncIpcClient::new(ScriptTransport::new(responses), Duration::from_secs(1))
        .expect("positive timeout");
    let request = client
        .send_request(MessageType::HeartbeatReq, b"request")
        .expect("request");

    assert_eq!(poll_ready(client.recv_event()).expect("event"), event);
    assert_eq!(
        poll_ready(client.recv_response_for(request)).expect("response"),
        (MessageType::HeartbeatResp, b"response".to_vec())
    );
}

#[test]
fn event_header_and_envelope_identifiers_must_match() {
    let event = RemoteEventPayload::new(1, "test.value", [0x9a, 0xbc]).expect("event");
    let response = frame(
        MessageType::TelemetryStreamEvent,
        2,
        &event.encode().expect("event payload"),
    );
    let mut client = AsyncIpcClient::new(ScriptTransport::new([response]), Duration::from_secs(1))
        .expect("positive timeout");
    assert_eq!(
        poll_ready(client.recv_event())
            .expect_err("mismatched event identifiers")
            .code,
        ErrorCode::SequenceMismatch
    );
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
    let mut client = AsyncIpcClient::new(ScriptTransport::new([response]), Duration::from_secs(1))
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
    let responses = [
        frame(MessageType::HeartbeatResp, 1, b"late"),
        frame(MessageType::HeartbeatResp, 2, b"active"),
    ];
    let mut client = AsyncIpcClient::new(ScriptTransport::new(responses), Duration::from_secs(1))
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
        poll_ready(client.recv_response()).expect("active response"),
        (RequestId(2), MessageType::HeartbeatResp, b"active".to_vec())
    );
    assert_eq!(client.pending_request_count(), 0);
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
    let mut client = AsyncIpcClient::new(ScriptTransport::new([response]), Duration::from_secs(1))
        .expect("positive timeout");
    let active = token([9; 16]);
    client.set_active_token(active.clone());
    let first = client
        .send_request(MessageType::HeartbeatReq, b"first")
        .expect("first request");
    let second = client
        .send_request(MessageType::HeartbeatReq, b"second")
        .expect("second request");

    let error = poll_ready(client.recv_response_for(first)).expect_err("missing first response");
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
