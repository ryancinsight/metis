//! Exercises native audit snapshot recovery and wrong-key rejection.

use metis_backend::BackendService;
use metis_backend::audit::{AuditCheckpointKey, FileAuditStore};
use metis_backend::clinical::SafetyEnvelope;
use metis_backend::service::SystemClock;
use metis_core::crc32;
use metis_core::protocol::{FrameHeader, HandshakeRequestPayload, MessageType, PROTOCOL_VERSION};
use metis_ipc::IpcHandler;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args()
        .nth(1)
        .ok_or("usage: audit_recovery <snapshot-directory>")?;
    let key = AuditCheckpointKey::new([0x5a; 32]);
    let store = FileAuditStore::open(&directory, key.clone())?;
    let mut service = BackendService::with_persistent_audit(
        [7; 32],
        SafetyEnvelope::default(),
        SystemClock::default(),
        store,
    )?;
    let payload = HandshakeRequestPayload {
        client_version: PROTOCOL_VERSION,
        client_process_id: 1,
        principal_id: [3; 16],
    }
    .encode();
    let header = FrameHeader {
        msg_type: MessageType::HandshakeReq,
        sequence_id: 1,
        payload_crc32: crc32(&payload),
        payload_len: u32::try_from(payload.len())?,
    };
    let response = service.handle_request(&header, &payload)?;
    assert_eq!(response.0, MessageType::HandshakeResp);
    let before = service.ledger().records().len();
    drop(service);

    let store = FileAuditStore::open(&directory, key.clone())?;
    let recovered = BackendService::with_persistent_audit(
        [7; 32],
        SafetyEnvelope::default(),
        SystemClock::default(),
        store,
    )?;
    let after = recovered.ledger().records().len();
    println!("Recovered {after} audit record(s) after restart (before={before})");

    let mut wrong_key = FileAuditStore::open(&directory, AuditCheckpointKey::new([0x6b; 32]))?;
    let error = wrong_key
        .load()
        .expect_err("a wrong checkpoint key must fail closed");
    println!("Wrong-key probe denied: {}", error.code.as_str());
    Ok(())
}
