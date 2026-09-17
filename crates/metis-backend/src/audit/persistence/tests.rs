use crate::audit::{AuditCheckpointKey, AuditEvent, AuditLedger, FileAuditStore};
use crate::clinical::SafetyEnvelope;
use metis_core::crc32;
use metis_core::error::ErrorCode;
use metis_core::protocol::{FrameHeader, HandshakeRequestPayload, MessageType, PROTOCOL_VERSION};
use metis_ipc::IpcHandler;
use metis_ipc::server::{FailureContext, RequestIdentity};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::audit::persistence::{HEADER_SIZE, SLOT_NAMES};

const KEY: AuditCheckpointKey = AuditCheckpointKey::new([0x5a; 32]);

fn temporary_directory() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("metis-audit-{stamp}"))
}

fn sample_ledger() -> AuditLedger {
    let mut ledger = AuditLedger::new();
    ledger
        .record(
            42,
            [3; 16],
            AuditEvent::Processed(RequestIdentity {
                message_type: MessageType::HeartbeatReq,
                sequence: 1,
            }),
            None,
        )
        .expect("record");
    ledger
        .record(
            43,
            [3; 16],
            AuditEvent::Failure(FailureContext::Request(RequestIdentity {
                message_type: MessageType::ClinicalCalcReq,
                sequence: 2,
            })),
            Some(ErrorCode::InvalidPatientWeight),
        )
        .expect("record");
    ledger
}

#[test]
fn snapshot_round_trip_preserves_records_and_generation() {
    let directory = temporary_directory();
    let mut store = FileAuditStore::open(&directory, KEY.clone()).expect("open");
    let ledger = sample_ledger();
    store.persist(&ledger).expect("persist");
    assert_eq!(store.generation(), 1);
    let mut reopened = FileAuditStore::open(&directory, KEY.clone()).expect("reopen");
    let recovered = reopened.load().expect("load");
    assert_eq!(recovered.records(), ledger.records());
    assert_eq!(reopened.generation(), 1);
    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn wrong_key_and_tamper_fail_closed() {
    let directory = temporary_directory();
    let mut store = FileAuditStore::open(&directory, KEY.clone()).expect("open");
    store.persist(&sample_ledger()).expect("persist");
    let bytes = fs::read(directory.join(SLOT_NAMES[0])).expect("read snapshot");
    let mut tampered = bytes;
    let index = HEADER_SIZE + 4;
    tampered[index] ^= 1;
    fs::write(directory.join(SLOT_NAMES[0]), tampered).expect("tamper");
    let mut wrong = FileAuditStore::open(&directory, AuditCheckpointKey::new([0x6b; 32]))
        .expect("wrong key open");
    assert_eq!(
        wrong.load().expect_err("wrong key").code,
        ErrorCode::ChecksumMismatch
    );
    let mut reopened = FileAuditStore::open(&directory, KEY.clone()).expect("reopen");
    assert_eq!(
        reopened.load().expect_err("tamper").code,
        ErrorCode::ChecksumMismatch
    );
    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn second_slot_keeps_a_previous_authenticated_generation() {
    let directory = temporary_directory();
    let mut store = FileAuditStore::open(&directory, KEY.clone()).expect("open");
    let first = sample_ledger();
    store.persist(&first).expect("first");
    let mut second = first.clone();
    second
        .record(
            44,
            [3; 16],
            AuditEvent::Failure(FailureContext::Receive),
            Some(ErrorCode::TransportBroken),
        )
        .expect("second record");
    store.persist(&second).expect("second");
    let mut reopened = FileAuditStore::open(&directory, KEY.clone()).expect("reopen");
    let recovered = reopened.load().expect("load");
    assert_eq!(recovered.records(), second.records());
    assert_eq!(reopened.generation(), 2);
    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn persistent_service_recovers_a_real_handshake_record() {
    let directory = temporary_directory();
    let store = FileAuditStore::open(&directory, KEY.clone()).expect("open");
    let mut service = crate::BackendService::with_persistent_audit(
        [7; 32],
        SafetyEnvelope::default(),
        crate::service::SystemClock::default(),
        store,
    )
    .expect("service");
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
        payload_len: u32::try_from(payload.len()).expect("payload fits"),
    };
    service
        .handle_request(&header, &payload)
        .expect("handshake");
    drop(service);
    let store = FileAuditStore::open(&directory, KEY.clone()).expect("reopen");
    let recovered = crate::BackendService::with_persistent_audit(
        [7; 32],
        SafetyEnvelope::default(),
        crate::service::SystemClock::default(),
        store,
    )
    .expect("recovered service");
    assert_eq!(recovered.ledger().records().len(), 1);
    assert_eq!(recovered.ledger().records()[0].sequence_id, 1);
    fs::remove_dir_all(directory).expect("cleanup");
}
