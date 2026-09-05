//! Interface contract verification tests for Metis binary IPC wire protocol.

use metis_core::capability::{CapabilityScope, CapabilityToken};
use metis_core::crypto::crc32;
use metis_core::error::ErrorCode;
use metis_core::protocol::{
    ClinicalCalcRequestPayload, ClinicalCalcResponsePayload, FrameHeader, HEADER_SIZE,
    HandshakeRequestPayload, MAX_PAYLOAD_SIZE, MessageType, PROTOCOL_VERSION, build_frame,
};
use metis_ipc::frame::read_frame;

#[test]
fn test_frame_header_encode_decode() {
    let header = FrameHeader {
        msg_type: MessageType::ClinicalCalcReq,
        sequence_id: 42,
        payload_crc32: 0x1234_5678,
        payload_len: 256,
    };
    let encoded = header.encode();
    assert_eq!(encoded.len(), HEADER_SIZE);

    let decoded = FrameHeader::decode(&encoded).expect("Failed to decode valid header");
    assert_eq!(decoded.msg_type, MessageType::ClinicalCalcReq);
    assert_eq!(decoded.sequence_id, 42);
    assert_eq!(decoded.payload_crc32, 0x1234_5678);
    assert_eq!(decoded.payload_len, 256);
}

#[test]
fn test_frame_header_magic_mismatch() {
    let mut header_bytes = [0u8; HEADER_SIZE];
    header_bytes[0..4].copy_from_slice(b"BAD!");
    let err = FrameHeader::decode(&header_bytes).expect_err("invalid input must be rejected");
    assert_eq!(err.code, ErrorCode::MagicMismatch);
}

#[test]
fn test_frame_header_version_mismatch() {
    let header = FrameHeader {
        msg_type: MessageType::HandshakeReq,
        sequence_id: 1,
        payload_crc32: 0,
        payload_len: 0,
    };
    let mut bytes = header.encode();
    bytes[4] = 0x99; // corrupt version
    let err = FrameHeader::decode(&bytes).expect_err("invalid input must be rejected");
    assert_eq!(err.code, ErrorCode::VersionMismatch);
}

#[test]
fn test_frame_payload_too_large() {
    let header = FrameHeader {
        msg_type: MessageType::HandshakeReq,
        sequence_id: 1,
        payload_crc32: 0,
        payload_len: u32::try_from(MAX_PAYLOAD_SIZE).expect("wire limit fits u32") + 1,
    };
    let bytes = header.encode();
    let err = FrameHeader::decode(&bytes).expect_err("invalid input must be rejected");
    assert_eq!(err.code, ErrorCode::PayloadTooLarge);
}

#[test]
fn test_handshake_payload_roundtrip() {
    let req = HandshakeRequestPayload {
        client_version: PROTOCOL_VERSION,
        client_process_id: 1024,
        principal_id: [0x11; 16],
    };
    let encoded = req.encode();
    let decoded = HandshakeRequestPayload::decode(&encoded).expect("Decode failed");
    assert_eq!(decoded, req);
}

#[test]
fn test_clinical_calc_request_roundtrip() {
    let token = CapabilityToken {
        token_id: 999,
        principal_id: [0x22; 16],
        scope: CapabilityScope::SUBMIT_CALCULATION,
        issued_at_secs: 1_000_000,
        expires_at_secs: 1_003_600,
        nonce: 12345,
        signature: [0xAA; 32],
    };
    let req = ClinicalCalcRequestPayload {
        token,
        patient_id: "PT-VERIFY-1".to_string(),
        weight_kg: 68.5,
        concentration_mg_ml: 2.0,
        target_dose_mcg_kg_min: 0.25,
    };
    let encoded = req.encode().expect("bounded request");
    let decoded = ClinicalCalcRequestPayload::decode(&encoded).expect("Decode failed");
    assert_eq!(decoded, req);
}

#[test]
fn test_clinical_calc_response_roundtrip() {
    let resp = ClinicalCalcResponsePayload {
        audit_sequence_id: 7,
        rate_ml_hr: 51.375,
        drug_rate_mg_hr: 102.75,
        is_pediatric: false,
        result_signature: [0xCC; 32],
    };
    let encoded = resp.encode();
    let decoded = ClinicalCalcResponsePayload::decode(&encoded).expect("Decode failed");
    assert_eq!(decoded, resp);
}

#[test]
fn test_crc32_checksum_validation_in_frame() {
    let payload = b"critical-medical-payload-data";
    let mut frame = build_frame(MessageType::ClinicalCalcReq, 10, payload).expect("bounded frame");

    // Read valid frame
    let mut slice = &frame[..];
    let (header, decoded_payload) = read_frame(&mut slice).expect("Valid frame failed");
    assert_eq!(decoded_payload, payload);
    assert_eq!(header.payload_crc32, crc32(payload));

    // Corrupt one payload byte
    let last_idx = frame.len() - 1;
    frame[last_idx] ^= 0xFF;
    let mut corrupt_slice = &frame[..];
    let err = read_frame(&mut corrupt_slice).expect_err("invalid input must be rejected");
    assert_eq!(err.code, ErrorCode::ChecksumMismatch);
}
