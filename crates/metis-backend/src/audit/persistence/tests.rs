use crate::audit::{AuditCheckpointKey, AuditEvent, AuditLedger, FileAuditStore};
use crate::clinical::SafetyEnvelope;
use metis_core::crc32;
use metis_core::error::ErrorCode;
use metis_core::protocol::{FrameHeader, HandshakeRequestPayload, MessageType, PROTOCOL_VERSION};
use metis_ipc::IpcHandler;
use metis_ipc::server::{FailureContext, RequestIdentity};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::audit::persistence::{HEADER_SIZE, SLOT_NAMES};

const KEY: AuditCheckpointKey = AuditCheckpointKey::new([0x5a; 32]);

fn temporary_directory() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    // (process, sequence) is injective within this test process. Atomic
    // create_dir reserves the path against stale fixtures from reused PIDs;
    // clock resolution is not an ownership or uniqueness guarantee.
    loop {
        let sequence = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .expect("test directory sequence exhausted");
        let directory =
            std::env::temp_dir().join(format!("metis-audit-{}-{sequence}", std::process::id()));
        match fs::create_dir(&directory) {
            Ok(()) => return directory,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => panic!("reserve audit test directory: {error}"),
        }
    }
}

#[test]
fn concurrent_stores_keep_distinct_authenticated_snapshots() {
    let _trace = tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_test_writer()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .finish(),
    );
    let _test = tracing::info_span!("audit_fixture_isolation").entered();
    let dispatch = tracing::dispatcher::get_default(Clone::clone);
    let parent = tracing::Span::current();
    let barrier = std::sync::Barrier::new(SLOT_NAMES.len());
    let snapshots = std::thread::scope(|scope| {
        let handles: Vec<_> = SLOT_NAMES
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let barrier = &barrier;
                let dispatch = &dispatch;
                let parent = &parent;
                scope.spawn(move || {
                    let _trace = tracing::dispatcher::set_default(dispatch);
                    let _parent = parent.enter();
                    let (directory, mut store, mut ledger) = tracing::info_span!("arrange", index)
                        .in_scope(|| {
                            let directory = temporary_directory();
                            let store =
                                FileAuditStore::open(&directory, KEY.clone()).expect("open");
                            (directory, store, sample_ledger())
                        });
                    if index == 1 {
                        ledger
                            .record(
                                44,
                                [3; 16],
                                AuditEvent::Failure(FailureContext::Receive),
                                Some(ErrorCode::TransportBroken),
                            )
                            .expect("distinct snapshot");
                    }
                    tracing::info_span!("act", index).in_scope(|| {
                        store.persist(&ledger).expect("persist");
                        barrier.wait();
                        let mut reopened =
                            FileAuditStore::open(&directory, KEY.clone()).expect("reopen");
                        let recovered = reopened.load().expect("load");
                        tracing::info_span!("assert", index).in_scope(|| {
                            assert_eq!(recovered.records(), ledger.records());
                            assert_eq!(reopened.generation(), 1);
                        });
                    });
                    directory
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("snapshot worker"))
            .collect::<Vec<_>>()
    });
    assert_ne!(snapshots[0], snapshots[1]);
    for directory in snapshots {
        fs::remove_dir_all(directory).expect("cleanup");
    }
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
