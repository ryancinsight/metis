//! Sequence, correlation, capability, and event-queue behaviour.

use super::*;
use crate::MemoryTransport;
use metis_core::protocol::{TargetCapability, TargetPlatform};

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
    let catalog =
        CapabilityCatalogPayload::new([MessageType::CapabilityReq, MessageType::ClinicalCalcReq])
            .expect("catalog");
    let decoded = decode_capability_response(
        MessageType::CapabilityResp,
        &catalog.encode().expect("encoded catalog"),
    )
    .expect("catalog response");
    assert_eq!(decoded, catalog);
}

#[test]
fn target_capability_discovery_correlates_and_decodes_the_host_descriptor() {
    let descriptor = TargetCapabilityPayload::new(
        TargetPlatform::Windows,
        [
            TargetCapability::NativeProcess,
            TargetCapability::BrowserWebSocket,
        ],
    )
    .expect("descriptor");
    let (transport, mut peer) = MemoryTransport::pair();
    peer.send_message(
        MessageType::TargetCapabilityResp,
        1,
        &descriptor.encode().expect("encoded descriptor"),
    )
    .expect("response");
    let mut client = IpcClient::new(transport);
    assert_eq!(
        client
            .discover_target_capabilities()
            .expect("target descriptor"),
        descriptor
    );
    let (header, payload) = peer.recv_message().expect("request");
    assert_eq!(header.msg_type, MessageType::TargetCapabilityReq);
    assert_eq!(header.sequence_id, 1);
    assert!(payload.is_empty());
}

#[test]
fn target_capability_discovery_rejects_a_descriptor_version_mismatch() {
    let descriptor = TargetCapabilityPayload::native_service();
    let mut payload = descriptor.encode().expect("encoded descriptor");
    payload[..2].copy_from_slice(&0x0200_u16.to_be_bytes());
    let error = decode_target_capability_response(MessageType::TargetCapabilityResp, &payload)
        .expect_err("version mismatch");
    assert!(matches!(
        error,
        TargetCapabilityError::Local(error) if error.code == ErrorCode::VersionMismatch
    ));
}

#[test]
fn target_capability_discovery_preserves_a_peer_rejection() {
    let rejection = ErrorResponsePayload {
        error_code: ErrorCode::MissingCapability as u16,
        message: ErrorCode::MissingCapability.as_str().to_owned(),
    };
    let error = decode_target_capability_response(
        MessageType::ErrorResp,
        &rejection.encode().expect("error payload"),
    )
    .expect_err("peer rejection");
    assert_eq!(error, TargetCapabilityError::Remote(rejection));
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
fn invokes_a_plugin_and_decodes_its_bounded_response() {
    let (transport, mut peer) = MemoryTransport::pair();
    let token = CapabilityToken::issue(
        7,
        [1; 16],
        metis_core::capability::CapabilityScope::UI_RENDER,
        10,
        20,
        30,
        b"test key",
    );
    let request = PluginInvocationPayload::new(token, "viewer", "open", [1, 4, 9])
        .expect("plugin invocation");
    let response = PluginInvocationResponsePayload::new([2, 5, 10])
        .expect("plugin response")
        .encode()
        .expect("encoded response");
    peer.send_message(MessageType::PluginInvokeResp, 1, &response)
        .expect("plugin response frame");

    let mut client = IpcClient::new(transport);
    let response = client.invoke_plugin(&request).expect("plugin response");
    assert_eq!(response.body(), [2, 5, 10]);
    let (header, payload) = peer.recv_message().expect("request frame");
    assert_eq!(header.msg_type, MessageType::PluginInvokeReq);
    assert_eq!(PluginInvocationPayload::decode(&payload), Ok(request));
}

#[test]
fn plugin_invocation_preserves_a_typed_remote_rejection() {
    let (transport, mut peer) = MemoryTransport::pair();
    let token = CapabilityToken::issue(
        7,
        [1; 16],
        metis_core::capability::CapabilityScope::UI_RENDER,
        10,
        20,
        30,
        b"test key",
    );
    let request =
        PluginInvocationPayload::new(token, "viewer", "open", []).expect("plugin invocation");
    let rejection = ErrorResponsePayload {
        error_code: ErrorCode::PluginNotFound as u16,
        message: ErrorCode::PluginNotFound.as_str().to_owned(),
    };
    peer.send_message(
        MessageType::ErrorResp,
        1,
        &rejection.encode().expect("error payload"),
    )
    .expect("rejection frame");
    let mut client = IpcClient::new(transport);
    assert_eq!(
        client.invoke_plugin(&request),
        Err(PluginInvocationError::Remote(rejection))
    );
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
