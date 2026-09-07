//! Demonstration arithmetic and adversarial backend-session contracts.

use metis_backend::BackendService;
use metis_backend::audit::AuditEvent;
use metis_backend::clinical::{
    DrugConcentrationMgMl, PatientWeightKg, SafetyEnvelope, TargetDoseRate, calculate_infusion_rate,
};
use metis_backend::service::{Clock, ClockReading, SESSION_LIFETIME};
use metis_core::capability::CapabilityToken;
use metis_core::error::{ErrorCode, Result};
use metis_core::host::{HostOrigin, HostPolicy, HostSessionId, WindowId};
use metis_core::protocol::{
    ClinicalCalcRequestPayload, ClinicalCalcResponsePayload, ErrorResponsePayload, FrameHeader,
    HandshakeRequestPayload, HandshakeResponsePayload, MessageType, PROTOCOL_VERSION,
};
use metis_ipc::server::{FailureContext, IpcHandler, IpcServer, RequestIdentity};
use metis_ipc::transport::{IpcTransport, MemoryTransport};
use moirai_crypto::hmac_sha256;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone)]
struct TestClock(Rc<Cell<ClockReading>>);

impl Clock for TestClock {
    fn now(&self) -> Result<ClockReading> {
        Ok(self.0.get())
    }
}

fn service() -> (BackendService<TestClock>, TestClock) {
    let clock = TestClock(Rc::new(Cell::new(ClockReading {
        unix_time: Duration::from_secs(1_700_000_000),
        monotonic: Duration::ZERO,
    })));
    (
        BackendService::with_clock([7; 32], SafetyEnvelope::default(), clock.clone()),
        clock,
    )
}

fn service_with_policy(policy: HostPolicy) -> (BackendService<TestClock>, TestClock, HostPolicy) {
    let clock = TestClock(Rc::new(Cell::new(ClockReading {
        unix_time: Duration::from_secs(1_700_000_000),
        monotonic: Duration::ZERO,
    })));
    (
        BackendService::with_clock_and_policy(
            [7; 32],
            SafetyEnvelope::default(),
            clock.clone(),
            policy.clone(),
        ),
        clock,
        policy,
    )
}

fn header(msg_type: MessageType, sequence_id: u64, payload: &[u8]) -> FrameHeader {
    FrameHeader {
        msg_type,
        sequence_id,
        payload_crc32: metis_core::crypto::crc32(payload),
        payload_len: u32::try_from(payload.len()).expect("bounded fixture"),
    }
}

fn handshake(service: &mut BackendService<TestClock>, principal: [u8; 16]) -> CapabilityToken {
    let payload = HandshakeRequestPayload {
        client_version: PROTOCOL_VERSION,
        client_process_id: 1,
        principal_id: principal,
    }
    .encode();
    let (kind, bytes) = service
        .handle_request(&header(MessageType::HandshakeReq, 1, &payload), &payload)
        .expect("dispatch");
    assert_eq!(kind, MessageType::HandshakeResp);
    HandshakeResponsePayload::decode(&bytes)
        .expect("response")
        .initial_token
}

fn request(token: CapabilityToken) -> ClinicalCalcRequestPayload {
    ClinicalCalcRequestPayload {
        token,
        patient_id: "confidential-patient".to_owned(),
        weight_kg: 25.0,
        concentration_mg_ml: 1.5,
        target_dose_mcg_kg_min: 0.5,
    }
}

fn error(
    service: &mut BackendService<TestClock>,
    sequence: u64,
    kind: MessageType,
    payload: &[u8],
) -> u16 {
    let (kind, bytes) = service
        .handle_request(&header(kind, sequence, payload), payload)
        .expect("dispatch");
    assert_eq!(kind, MessageType::ErrorResp);
    ErrorResponsePayload::decode(&bytes)
        .expect("error payload")
        .error_code
}

#[test]
fn configured_boundaries_and_dimensional_reference() {
    let weight = PatientWeightKg::new(25.0).expect("weight");
    let concentration = DrugConcentrationMgMl::new(1.5).expect("concentration");
    let dose = TargetDoseRate::new(0.5).expect("dose");
    let envelope = SafetyEnvelope::new(1.0, 0.5, 35.0).expect("envelope");
    let result = calculate_infusion_rate(weight, concentration, dose, &envelope)
        .expect("inclusive boundary");
    // Every intermediate is representable: .5 * 25 * 60 / (1.5 * 1000)
    // = 750 / 1500 = .5, so this fixture requires bitwise equality.
    assert_eq!(result.rate_ml_hr.to_bits(), 0.5_f64.to_bits());
    assert_eq!(result.drug_rate_mg_hr.to_bits(), 0.75_f64.to_bits());
    assert!(result.is_pediatric);
    let above = calculate_infusion_rate(
        weight,
        concentration,
        TargetDoseRate::new(1.0).expect("dose"),
        &envelope,
    )
    .expect_err("above pediatric limit");
    assert_eq!(above.code, ErrorCode::PediatricRateExceeded);
    let adult = SafetyEnvelope::new(1.0, 0.5, 25.0).expect("envelope");
    assert!(
        !calculate_infusion_rate(weight, concentration, dose, &adult)
            .expect("threshold")
            .is_pediatric
    );
}

#[test]
fn rejects_invalid_envelopes_and_numeric_inputs() {
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -1.0] {
        for (adult, pediatric, threshold) in [
            (invalid, 0.5, 35.0),
            (1.0, invalid, 35.0),
            (1.0, 0.5, invalid),
        ] {
            assert_eq!(
                SafetyEnvelope::new(adult, pediatric, threshold)
                    .expect_err("invalid envelope")
                    .code,
                ErrorCode::ClinicalInterlockBlocked
            );
        }
    }
    assert_eq!(
        SafetyEnvelope::new(1.0, 2.0, 35.0)
            .expect_err("unordered ceilings")
            .code,
        ErrorCode::ClinicalInterlockBlocked
    );
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            PatientWeightKg::new(invalid).expect_err("weight").code,
            ErrorCode::NumericInstability
        );
        assert_eq!(
            DrugConcentrationMgMl::new(invalid)
                .expect_err("concentration")
                .code,
            ErrorCode::NumericInstability
        );
        assert_eq!(
            TargetDoseRate::new(invalid).expect_err("dose").code,
            ErrorCode::NumericInstability
        );
    }
}

#[test]
fn preserves_reference_rates_and_adult_interlock() {
    for (weight, concentration, dose, expected, pediatric) in [
        (70.0, 4.0, 0.05, 0.0525_f64, false),
        (10.0, 1.0, 5.0, 3.0, true),
    ] {
        let result = calculate_infusion_rate(
            PatientWeightKg::new(weight).expect("weight"),
            DrugConcentrationMgMl::new(concentration).expect("concentration"),
            TargetDoseRate::new(dose).expect("dose"),
            &SafetyEnvelope::default(),
        )
        .expect("reference");
        // Four arithmetic roundings, one decimal-input rounding and one
        // reference rounding give gamma(6) with u = EPSILON/2. All values
        // stay normal and the multiplication/division condition factors are 1.
        let unit = f64::EPSILON / 2.0;
        let bound = (6.0 * unit / (1.0 - 6.0 * unit)) * expected.abs();
        assert!((result.rate_ml_hr - expected).abs() <= bound);
        assert_eq!(result.is_pediatric, pediatric);
    }
    for (weight, concentration, dose, code) in [
        (100.0, 0.1, 10.0, ErrorCode::RateExceedsSafetyEnvelope),
        (10.0, 1.0, 90.0, ErrorCode::PediatricRateExceeded),
    ] {
        let error = calculate_infusion_rate(
            PatientWeightKg::new(weight).expect("weight"),
            DrugConcentrationMgMl::new(concentration).expect("concentration"),
            TargetDoseRate::new(dose).expect("dose"),
            &SafetyEnvelope::default(),
        )
        .expect_err("ceiling");
        assert_eq!(error.code, code);
    }
    for weight in [-5.0, 0.0, 1000.0] {
        assert_eq!(
            PatientWeightKg::new(weight)
                .expect_err("weight bounds")
                .code,
            ErrorCode::InvalidPatientWeight
        );
    }
}

#[test]
fn capability_expires_on_monotonic_time_even_if_utc_stalls() {
    let (mut service, clock) = service();
    let token = handshake(&mut service, [1; 16]);
    let bytes = request(token).encode().expect("request");
    let mut reading = clock.0.get();
    reading.monotonic = SESSION_LIFETIME;
    clock.0.set(reading);
    assert_eq!(
        error(&mut service, 2, MessageType::ClinicalCalcReq, &bytes),
        ErrorCode::CapabilityExpired as u16
    );
    assert_eq!(
        service.ledger().records().back().expect("audit").outcome,
        Some(ErrorCode::CapabilityExpired)
    );
}

#[test]
fn clock_rollback_remains_failed_after_clock_recovers() {
    let (mut service, clock) = service();
    let bytes = request(handshake(&mut service, [1; 16]))
        .encode()
        .expect("request");
    let original = clock.0.get();
    clock.0.set(ClockReading {
        unix_time: original
            .unix_time
            .checked_sub(Duration::from_secs(1))
            .expect("fixture time exceeds one second"),
        monotonic: Duration::from_secs(1),
    });
    assert_eq!(
        error(&mut service, 2, MessageType::ClinicalCalcReq, &bytes),
        ErrorCode::CapabilityExpired as u16
    );
    clock.0.set(original);
    assert_eq!(
        error(&mut service, 3, MessageType::ClinicalCalcReq, &bytes),
        ErrorCode::CapabilityExpired as u16
    );
}

#[test]
fn session_rejects_another_principal_and_replayed_sequence() {
    let (mut first, _) = service();
    let foreign_token = handshake(&mut first, [1; 16]);
    let (mut second, _) = service();
    let own_token = handshake(&mut second, [2; 16]);
    let bytes = request(foreign_token).encode().expect("request");
    assert_eq!(
        error(&mut second, 2, MessageType::ClinicalCalcReq, &bytes),
        ErrorCode::InvalidPrincipal as u16
    );
    let own_bytes = request(own_token).encode().expect("request");
    assert_eq!(
        error(&mut second, 2, MessageType::ClinicalCalcReq, &own_bytes),
        ErrorCode::ReplayDetected as u16
    );
    let (kind, _) = second
        .handle_request(
            &header(MessageType::ClinicalCalcReq, 3, &own_bytes),
            &own_bytes,
        )
        .expect("dispatch");
    assert_eq!(kind, MessageType::ClinicalCalcResp);
}

#[test]
fn backend_handshake_binds_issued_token_to_configured_host_policy() {
    let policy = HostPolicy::new(
        HostOrigin::try_from("https://viewer.example").expect("origin"),
        WindowId::new(9).expect("window"),
    );
    let (mut service, _, policy) = service_with_policy(policy);
    let principal = [0x55; 16];
    let token = handshake(&mut service, principal);
    let context = policy.context_for(HostSessionId::new(principal).expect("session"));
    token
        .verify_for_host(
            metis_core::capability::CapabilityScope::SUBMIT_CALCULATION,
            1_700_000_001,
            &[7; 32],
            &context,
        )
        .expect("configured policy binding");
    let native_context =
        HostPolicy::native().context_for(HostSessionId::new(principal).expect("session"));
    assert_eq!(
        token
            .verify_for_host(
                metis_core::capability::CapabilityScope::SUBMIT_CALCULATION,
                1_700_000_001,
                &[7; 32],
                &native_context,
            )
            .expect_err("retargeted policy")
            .code,
        ErrorCode::InvalidCapabilitySignature
    );
}

#[test]
fn every_dispatched_outcome_is_recorded_without_patient_text() {
    let (mut service, _) = service();
    assert_eq!(
        error(&mut service, 1, MessageType::HandshakeReq, b"bad"),
        ErrorCode::MalformedPayload as u16
    );
    let (kind, bytes) = service
        .handle_request(&header(MessageType::HeartbeatReq, 2, b""), b"")
        .expect("heartbeat");
    assert_eq!((kind, bytes), (MessageType::HeartbeatResp, Vec::new()));
    assert_eq!(
        error(
            &mut service,
            3,
            MessageType::AuditQueryReq,
            b"confidential-patient"
        ),
        ErrorCode::UnexpectedMessageType as u16
    );
    let records = service.ledger().records();
    assert_eq!(records.len(), 3);
    assert_eq!(
        records
            .iter()
            .map(|entry| entry.outcome)
            .collect::<Vec<_>>(),
        vec![
            Some(ErrorCode::MalformedPayload),
            None,
            Some(ErrorCode::UnexpectedMessageType)
        ]
    );
    assert!(!format!("{records:?}").contains("confidential-patient"));
    assert_eq!(service.ledger().verify_chain(), Ok(()));
}

fn result_mac(sequence: u64, request: &[u8], response: &ClinicalCalcResponsePayload) -> [u8; 32] {
    let mut bytes = b"METIS-CALCULATION-RESULT-1".to_vec();
    bytes.extend_from_slice(&(MessageType::ClinicalCalcReq as u16).to_be_bytes());
    bytes.extend_from_slice(&sequence.to_be_bytes());
    bytes.extend_from_slice(
        &u32::try_from(request.len())
            .expect("fixture length")
            .to_be_bytes(),
    );
    bytes.extend_from_slice(request);
    bytes.extend_from_slice(&response.audit_sequence_id.to_be_bytes());
    bytes.extend_from_slice(&response.rate_ml_hr.to_be_bytes());
    bytes.extend_from_slice(&response.drug_rate_mg_hr.to_be_bytes());
    bytes.push(u8::from(response.is_pediatric));
    hmac_sha256(&[7; 32], &bytes)
}

#[test]
fn result_mac_binds_request_sequence_patient_and_every_result_field() {
    let (mut service, _) = service();
    let mut request = request(handshake(&mut service, [1; 16]));
    let bytes = request.encode().expect("request");
    let (kind, result) = service
        .handle_request(&header(MessageType::ClinicalCalcReq, 2, &bytes), &bytes)
        .expect("dispatch");
    assert_eq!(kind, MessageType::ClinicalCalcResp);
    let response = ClinicalCalcResponsePayload::decode(&result).expect("response");
    assert_eq!(response.result_signature, result_mac(2, &bytes, &response));
    assert_ne!(response.result_signature, result_mac(3, &bytes, &response));
    request.patient_id = "different-patient".to_owned();
    assert_ne!(
        response.result_signature,
        result_mac(2, &request.encode().expect("request"), &response)
    );
    let mut changed = response.clone();
    changed.is_pediatric = !changed.is_pediatric;
    assert_ne!(response.result_signature, result_mac(2, &bytes, &changed));
    changed = response.clone();
    changed.audit_sequence_id += 1;
    assert_ne!(response.result_signature, result_mac(2, &bytes, &changed));
    changed = response.clone();
    changed.rate_ml_hr = 1.0;
    assert_ne!(response.result_signature, result_mac(2, &bytes, &changed));
    changed = response.clone();
    changed.drug_rate_mg_hr = 1.0;
    assert_ne!(response.result_signature, result_mac(2, &bytes, &changed));
}

#[test]
fn wire_failures_are_audited_and_clean_eof_is_not() {
    let (mut service, _) = service();
    let (mut peer, transport) = MemoryTransport::pair();
    peer.send_frame(b"MET").expect("partial header");
    let mut server = IpcServer::new(transport);
    assert_eq!(
        server.step(&mut service).expect_err("truncation").code,
        ErrorCode::FrameTruncated
    );
    let event = service.ledger().records().back().expect("audit");
    assert_eq!(event.event, AuditEvent::Failure(FailureContext::Receive));
    assert_eq!(event.outcome, Some(ErrorCode::FrameTruncated));
    assert_eq!(event.timestamp_millis, 1_700_000_000_000);
    drop(peer);
    assert_eq!(server.step(&mut service), Ok(false));
    assert_eq!(service.ledger().records().len(), 1);
    assert_eq!(service.ledger().verify_chain(), Ok(()));
}

#[test]
fn application_rejections_and_server_replay_have_distinct_single_records() {
    let (mut service, _) = service();
    let (mut peer, transport) = MemoryTransport::pair();
    let mut server = IpcServer::new(transport);
    peer.send_message(MessageType::HandshakeReq, 1, b"bad")
        .expect("malformed payload");
    assert_eq!(server.step(&mut service), Ok(true));
    let (response, bytes) = peer.recv_message().expect("response");
    assert_eq!(response.msg_type, MessageType::ErrorResp);
    assert_eq!(
        ErrorResponsePayload::decode(&bytes)
            .expect("payload")
            .error_code,
        ErrorCode::MalformedPayload as u16
    );
    assert_eq!(service.ledger().records().len(), 1);
    let identity = RequestIdentity {
        message_type: MessageType::HandshakeReq,
        sequence: 1,
    };
    assert_eq!(
        service.ledger().records().back().expect("audit").event,
        AuditEvent::Processed(identity)
    );
    peer.send_message(MessageType::HandshakeReq, 1, b"bad")
        .expect("replay");
    assert_eq!(
        server.step(&mut service).expect_err("replay").code,
        ErrorCode::ReplayDetected
    );
    assert_eq!(service.ledger().records().len(), 2);
    let event = service.ledger().records().back().expect("audit");
    assert_eq!(
        event.event,
        AuditEvent::Failure(FailureContext::Request(identity))
    );
    assert_eq!(event.outcome, Some(ErrorCode::ReplayDetected));
    assert_eq!(service.ledger().verify_chain(), Ok(()));
}

#[test]
fn undelivered_response_records_processing_then_delivery_failure() {
    let (mut service, _) = service();
    let (mut peer, transport) = MemoryTransport::pair();
    peer.send_message(MessageType::HeartbeatReq, 1, b"")
        .expect("request");
    drop(peer);
    let mut server = IpcServer::new(transport);
    assert_eq!(
        server.step(&mut service).expect_err("write closed").code,
        ErrorCode::ConnectionClosed
    );
    let identity = RequestIdentity {
        message_type: MessageType::HeartbeatReq,
        sequence: 1,
    };
    assert_eq!(
        service
            .ledger()
            .records()
            .iter()
            .map(|record| (record.event, record.outcome))
            .collect::<Vec<_>>(),
        vec![
            (AuditEvent::Processed(identity), None),
            (
                AuditEvent::Failure(FailureContext::Response(identity)),
                Some(ErrorCode::ConnectionClosed)
            )
        ]
    );
    assert_eq!(service.ledger().verify_chain(), Ok(()));
}
