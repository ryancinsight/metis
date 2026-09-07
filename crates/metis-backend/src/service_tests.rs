use super::*;
use metis_core::crc32;
use metis_core::protocol::{
    CapabilityCatalogPayload, FrameHeader, HandshakeRequestPayload, MessageType, PROTOCOL_VERSION,
};

const KEY: [u8; 32] = [7; 32];
const PRINCIPAL: [u8; 16] = [3; 16];

fn header(message_type: MessageType, sequence: u64, payload: &[u8]) -> FrameHeader {
    FrameHeader {
        msg_type: message_type,
        sequence_id: sequence,
        payload_crc32: crc32(payload),
        payload_len: u32::try_from(payload.len()).expect("test payload fits wire field"),
    }
}

#[test]
fn capability_catalog_requires_handshake_and_advertises_supported_commands() {
    let mut service = BackendService::new(KEY, SafetyEnvelope::default());
    let empty = service
        .handle_request(&header(MessageType::CapabilityReq, 1, &[]), &[])
        .expect("typed missing-session response");
    assert_eq!(empty.0, MessageType::ErrorResp);
    let rejection = ErrorResponsePayload::decode(&empty.1).expect("error payload");
    assert_eq!(rejection.error_code, ErrorCode::MissingCapability as u16);

    let handshake = HandshakeRequestPayload {
        client_version: PROTOCOL_VERSION,
        client_process_id: 42,
        principal_id: PRINCIPAL,
    }
    .encode();
    let accepted = service
        .handle_request(
            &header(MessageType::HandshakeReq, 2, &handshake),
            &handshake,
        )
        .expect("handshake");
    assert_eq!(accepted.0, MessageType::HandshakeResp);

    let catalog = service
        .handle_request(&header(MessageType::CapabilityReq, 3, &[]), &[])
        .expect("catalog");
    assert_eq!(catalog.0, MessageType::CapabilityResp);
    let catalog = CapabilityCatalogPayload::decode(&catalog.1).expect("catalog payload");
    assert!(catalog.supports(MessageType::CapabilityReq));
    assert!(catalog.supports(MessageType::HeartbeatReq));
    assert!(catalog.supports(MessageType::ClinicalCalcReq));
    assert!(!catalog.supports(MessageType::AuditQueryReq));
}

#[test]
fn known_but_unadvertised_command_returns_a_typed_protocol_error() {
    let mut service = BackendService::new(KEY, SafetyEnvelope::default());
    let handshake = HandshakeRequestPayload {
        client_version: PROTOCOL_VERSION,
        client_process_id: 42,
        principal_id: PRINCIPAL,
    }
    .encode();
    service
        .handle_request(
            &header(MessageType::HandshakeReq, 1, &handshake),
            &handshake,
        )
        .expect("handshake");
    let response = service
        .handle_request(&header(MessageType::AuditQueryReq, 2, &[]), &[])
        .expect("typed unsupported response");
    assert_eq!(response.0, MessageType::ErrorResp);
    let error = ErrorResponsePayload::decode(&response.1).expect("error payload");
    assert_eq!(error.error_code, ErrorCode::UnexpectedMessageType as u16);
}
