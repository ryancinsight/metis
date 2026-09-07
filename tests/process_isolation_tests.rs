//! End-to-end presentation and backend exchange on a bounded memory transport.
use metis_backend::{BackendService, clinical::SafetyEnvelope};
use metis_core::error::ErrorCode;
use metis_core::protocol::{ClinicalCalcResponsePayload, MessageType};
use metis_frontend::{FormState, FrontendApp};
use metis_ipc::{
    client::HandshakeError,
    server::{IpcHandler, IpcServer},
    transport::{IpcTransport, MemoryTransport},
};
use metis_platform::framebuffer::{Color, Framebuffer};
use metis_ui_lang::layout::compute_layout;
use moirai_core::TaskSpawner;
use moirai_executor::ExecutorBuilder;

fn session_trace(
    request_count: usize,
    trace: impl FnOnce(&mut FrontendApp<MemoryTransport>),
) -> (FrontendApp<MemoryTransport>, BackendService) {
    let (front, back) = MemoryTransport::pair();
    let mut executor = ExecutorBuilder::new()
        .worker_threads(1)
        .async_threads(1)
        .build()
        .expect("Moirai executor");
    let server = executor
        .spawn_blocking(move || {
            let mut service = BackendService::new([0x77; 32], SafetyEnvelope::default());
            let mut server = IpcServer::new(back);
            for _ in 0..request_count {
                assert!(server.step(&mut service).expect("session step"));
            }
            service
        })
        .expect("admit backend task");
    let mut app = FrontendApp::new(front, 800, 600).expect("presentation");
    trace(&mut app);
    // Joining establishes that the server endpoint has closed, without sleeps.
    let service = server.join().expect("attached task").expect("backend task");
    executor.shutdown().expect("Moirai shutdown");
    service.ledger().verify_chain().expect("audit consistency");
    (app, service)
}

fn label(app: &FrontendApp<MemoryTransport>, id: &str) -> String {
    app.document()
        .find_element_by_id(id)
        .expect("form label")
        .text_content()
}

fn assert_current_pixels(app: &FrontendApp<MemoryTransport>) {
    let mut expected = Framebuffer::new(800, 600).expect("reference surface");
    expected.clear(Color::rgb(240, 244, 248));
    compute_layout(app.document(), 800, 600)
        .expect("current document layout")
        .render_to(&mut expected);
    // This checks state-to-frame synchronization, not rasterizer correctness.
    assert_eq!(app.framebuffer().pixels(), expected.pixels());
}

fn assert_no_result(app: &FrontendApp<MemoryTransport>, status: &str) {
    assert_eq!(label(app, "output-rate"), "Rate: No result");
    assert_eq!(label(app, "output-signature"), "Backend MAC: No result");
    assert_eq!(label(app, "output-status"), status);
    assert_current_pixels(app);
}

fn assert_result(app: &FrontendApp<MemoryTransport>, rate: f64, sequence: u64) {
    let FormState::Success(result) = app.state() else {
        panic!("expected backend success, got {:?}", app.state());
    };
    // Four arithmetic operations and input rounding: gamma(6) bounds relative error.
    let bound = 6.0 * f64::EPSILON / (1.0 - 6.0 * f64::EPSILON);
    assert!((result.rate_ml_hr - rate).abs() <= rate * bound);
    assert!((result.drug_rate_mg_hr - rate * 2.0).abs() <= rate * 2.0 * bound);
    assert_eq!(result.audit_sequence_id, sequence);
    assert!(!result.is_pediatric);
    assert_eq!(
        label(app, "output-rate"),
        format!("Rate: {rate:.3} mL/hr ({:.2} mg/hr)", rate * 2.0)
    );
    assert_eq!(
        label(app, "output-signature"),
        "Backend MAC: Present (not verified by frontend)"
    );
    assert_eq!(
        label(app, "output-status"),
        format!("Backend response (Audit Seq #{sequence})")
    );
    assert_current_pixels(app);
}

fn assert_clinical_event(app: &mut FrontendApp<MemoryTransport>, rate: f64, sequence: u64) {
    let event = app.recv_event().expect("backend clinical event");
    assert_eq!(event.name(), "clinical.result");
    assert_eq!(event.event_id().get(), sequence);
    let response = event
        .decode_as::<ClinicalCalcResponsePayload>()
        .expect("typed clinical event");
    assert_eq!(response.audit_sequence_id, sequence);
    let bound = 6.0 * f64::EPSILON / (1.0 - 6.0 * f64::EPSILON);
    assert!((response.rate_ml_hr - rate).abs() <= rate * bound);
    assert!((response.drug_rate_mg_hr - rate * 2.0).abs() <= rate * 2.0 * bound);
}

#[test]
fn form_edits_clear_result_before_rejection_and_recovery() {
    let (_, service) = session_trace(5, |app| {
        app.init(1234, [1; 16]).expect("handshake");
        app.set_inputs("demo", 60.0, 2.0, 0.2).expect("inputs");
        app.submit_calculation().expect("submission");
        assert_result(app, 0.36, 2);
        assert_clinical_event(app, 0.36, 2);
        let success_pixels = app.framebuffer().pixels().to_vec();
        app.set_inputs("demo", 80.0, 2.0, 0.2)
            .expect("edited inputs");
        assert_eq!(app.state(), &FormState::Idle);
        assert_eq!(label(app, "label-weight"), "Weight: 80.00 kg");
        assert_eq!(
            label(app, "output-rate"),
            "Rate: Awaiting Backend Calculation..."
        );
        assert_eq!(label(app, "output-status"), "Safety Status: Idle");
        assert_eq!(label(app, "output-signature"), "Backend MAC: None");
        assert_ne!(app.framebuffer().pixels(), success_pixels);
        assert_current_pixels(app);
        app.submit_calculation().expect("changed submission");
        assert_result(app, 0.48, 3);
        assert_clinical_event(app, 0.48, 3);
        app.set_inputs("demo", 0.0, 2.0, 0.2)
            .expect("invalid domain input");
        app.submit_calculation().expect("correlated rejection");
        let FormState::Rejected(error) = app.state() else {
            panic!("expected domain rejection, got {:?}", app.state());
        };
        assert_eq!(error.error_code, ErrorCode::InvalidPatientWeight as u16);
        assert_no_result(app, "Backend rejected request [0x3001]");
        app.set_inputs("demo", 60.0, 2.0, 0.2)
            .expect("corrected inputs");
        assert_eq!(app.state(), &FormState::Idle);
        app.submit_calculation().expect("recovery");
        assert_result(app, 0.36, 5);
        assert_clinical_event(app, 0.36, 5);
    });
    assert_eq!(
        service
            .ledger()
            .records()
            .iter()
            .map(|record| (record.sequence_id, record.outcome))
            .collect::<Vec<_>>(),
        [
            (1, None),
            (2, None),
            (3, None),
            (4, Some(ErrorCode::InvalidPatientWeight)),
            (5, None)
        ]
    );
}

#[test]
fn missing_capability_is_visible_and_handshake_recovers() {
    let (_, service) = session_trace(2, |app| {
        let error = app.submit_calculation().expect_err("handshake required");
        assert_eq!(error.code, ErrorCode::MissingCapability);
        assert_eq!(app.state(), &FormState::Failed(error));
        assert_no_result(app, "Request not sent [0x2001]");
        app.init(1234, [1; 16])
            .expect("handshake after local failure");
        app.set_inputs("demo", 60.0, 2.0, 0.2).expect("inputs");
        app.submit_calculation().expect("submission");
        assert_result(app, 0.36, 2);
    });
    assert_eq!(service.ledger().records().len(), 2);
}

#[test]
fn encoding_failure_preserves_session_without_sending_request() {
    let (_, service) = session_trace(3, |app| {
        app.init(1234, [1; 16]).expect("handshake");
        app.set_inputs("demo", 60.0, 2.0, 0.2).expect("inputs");
        app.submit_calculation().expect("first result");
        assert_result(app, 0.36, 2);
        let oversized_reference = "x".repeat(usize::from(u16::MAX) + 1);
        app.set_inputs(&oversized_reference, 60.0, 2.0, 0.2)
            .expect("domain input capture");
        let error = app.submit_calculation().expect_err("wire string bound");
        assert_eq!(error.code, ErrorCode::PayloadTooLarge);
        assert_eq!(app.state(), &FormState::Failed(error));
        assert_no_result(app, "Request not sent [0x1004]");
        assert_eq!(app.inputs().patient_id, oversized_reference);
        assert_eq!(
            label(app, "label-patient"),
            format!("Patient ID: {}...", "x".repeat(40))
        );
        app.set_inputs("demo", 60.0, 2.0, 0.2)
            .expect("bounded reference");
        app.submit_calculation().expect("same-session recovery");
        assert_result(app, 0.36, 3);
    });
    assert_eq!(service.ledger().records().len(), 3);
}

#[test]
fn closed_peer_replaces_success_and_new_session_recovers() {
    let (mut app, service) = session_trace(2, |app| {
        app.init(1234, [1; 16]).expect("handshake");
        app.set_inputs("demo", 60.0, 2.0, 0.2).expect("inputs");
        app.submit_calculation().expect("first result");
        assert_result(app, 0.36, 2);
    });
    let success_pixels = app.framebuffer().pixels().to_vec();
    let error = app.submit_calculation().expect_err("closed peer");
    assert_eq!(error.code, ErrorCode::ConnectionClosed);
    assert_eq!(app.state(), &FormState::Disconnected(error));
    assert_ne!(app.framebuffer().pixels(), success_pixels);
    assert_no_result(&app, "Connection failed [0x4002] - reconnect");
    assert_eq!(label(&app, "status-badge"), "SESSION CLOSED");
    assert_eq!(app.framebuffer().get_pixel(34, 58), Color::RED);
    assert_eq!(service.ledger().records().len(), 2);
    let (_, recovered) = session_trace(2, |app| {
        app.init(1234, [1; 16]).expect("new handshake");
        app.set_inputs("demo", 80.0, 2.0, 0.2).expect("new inputs");
        app.submit_calculation().expect("new session result");
        assert_result(app, 0.48, 2);
    });
    assert_eq!(recovered.ledger().records().len(), 2);
}

#[test]
fn repeated_handshake_clears_success_and_closes_session() {
    let (_, service) = session_trace(3, |app| {
        app.init(1234, [1; 16]).expect("handshake");
        app.set_inputs("demo", 60.0, 2.0, 0.2).expect("inputs");
        app.submit_calculation().expect("first result");
        assert_result(app, 0.36, 2);
        let error = app.init(1234, [1; 16]).expect_err("repeated handshake");
        let HandshakeError::Remote(rejection) = &error else {
            panic!("expected backend rejection, got {error:?}");
        };
        assert_eq!(
            rejection.error_code,
            ErrorCode::PrivilegeEscalationAttempt as u16
        );
        assert_eq!(app.state(), &FormState::SessionFailed(error));
        assert_no_result(app, "Session failed [0x2005] - reconnect");
        let error = app
            .submit_calculation()
            .expect_err("failed session stays closed");
        assert_eq!(error.code, ErrorCode::ConnectionClosed);
        assert_eq!(app.state(), &FormState::Disconnected(error));
        assert_no_result(app, "Connection failed [0x4002] - reconnect");
    });
    assert_eq!(
        service
            .ledger()
            .records()
            .iter()
            .map(|record| record.outcome)
            .collect::<Vec<_>>(),
        [None, None, Some(ErrorCode::PrivilegeEscalationAttempt)]
    );
}

#[test]
fn accepted_small_inputs_keep_their_value_on_screen() {
    let (_, service) = session_trace(2, |app| {
        app.init(1234, [1; 16]).expect("handshake");
        app.set_inputs("demo", 60.0, 0.001, 0.0001)
            .expect("small accepted inputs");
        assert_eq!(label(app, "label-conc"), "Drug Concentration: 0.001 mg/mL");
        assert_eq!(label(app, "label-dose"), "Target Dose: 0.0001 mcg/kg/min");
        assert_current_pixels(app);
        app.submit_calculation().expect("small-input calculation");
        let FormState::Success(result) = app.state() else {
            panic!("expected backend success, got {:?}", app.state());
        };
        // 60 kg * 0.0001 mcg/kg/min * 60 min/hr / 1000 mcg/mg = 0.00036 mg/hr.
        // Dividing by 0.001 mg/mL gives 0.36 mL/hr; gamma(6) includes input rounding.
        let bound = 6.0 * f64::EPSILON / (1.0 - 6.0 * f64::EPSILON);
        assert!((result.rate_ml_hr - 0.36).abs() <= 0.36 * bound);
        assert!((result.drug_rate_mg_hr - 0.00036).abs() <= 0.00036 * bound);
        assert_eq!(result.audit_sequence_id, 2);
        assert_eq!(
            label(app, "output-rate"),
            "Rate: 0.360 mL/hr (3.60e-4 mg/hr)"
        );
        assert_current_pixels(app);
    });
    assert_eq!(
        service
            .ledger()
            .records()
            .iter()
            .map(|record| record.outcome)
            .collect::<Vec<_>>(),
        [None, None]
    );
}

#[derive(Clone, Copy, Debug)]
enum ResponseFault {
    Sequence,
    Type,
    Truncated,
    Boolean,
    Rejection,
}

#[test]
fn response_faults_invalidate_success_and_stop_further_dispatch() {
    for (fault, expected) in [
        (ResponseFault::Sequence, ErrorCode::SequenceMismatch),
        (ResponseFault::Type, ErrorCode::UnexpectedMessageType),
        (ResponseFault::Truncated, ErrorCode::MalformedPayload),
        (ResponseFault::Boolean, ErrorCode::MalformedPayload),
        (ResponseFault::Rejection, ErrorCode::MalformedPayload),
    ] {
        let (front, mut back) = MemoryTransport::pair();
        let mut executor = ExecutorBuilder::new()
            .worker_threads(1)
            .async_threads(1)
            .build()
            .expect("Moirai executor");
        let server = executor
            .spawn_blocking(move || {
                let mut service = BackendService::new([0x77; 32], SafetyEnvelope::default());
                for request_sequence in 1..=3 {
                    let (header, payload) = back.recv_message().expect("real request");
                    assert_eq!(header.sequence_id, request_sequence);
                    let (mut kind, mut response) = service
                        .handle_request(&header, &payload)
                        .expect("real backend dispatch");
                    let mut response_sequence = header.sequence_id;
                    if request_sequence == 3 {
                        assert_eq!(kind, MessageType::ClinicalCalcResp);
                        match fault {
                            ResponseFault::Sequence => response_sequence += 1,
                            ResponseFault::Type => kind = MessageType::HeartbeatResp,
                            ResponseFault::Truncated => response.truncate(1),
                            // The canonical response stores three 8-byte values before its boolean.
                            ResponseFault::Boolean => response[3 * 8] = 2,
                            ResponseFault::Rejection => {
                                kind = MessageType::ErrorResp;
                                response.truncate(1);
                            }
                        }
                    }
                    back.send_message(kind, response_sequence, &response)
                        .expect("fault boundary");
                }
                // A fourth request would fail this assertion; closure proves the client retires
                // uncertain sessions rather than dispatching against a stale capability.
                assert_eq!(
                    back.recv_message()
                        .expect_err("frontend closes session")
                        .code,
                    ErrorCode::ConnectionClosed
                );
                service
            })
            .expect("admit backend task");
        let mut app = FrontendApp::new(front, 800, 600).expect("presentation");
        app.init(1234, [1; 16]).expect("handshake");
        app.set_inputs("demo", 60.0, 2.0, 0.2).expect("inputs");
        app.submit_calculation().expect("first result");
        assert_result(&app, 0.36, 2);
        let error = app.submit_calculation().expect_err("corrupted response");
        assert_eq!(error.code, expected, "{fault:?}");
        assert_eq!(app.state(), &FormState::Disconnected(error));
        assert_no_result(
            &app,
            &format!("Connection failed [0x{:04X}] - reconnect", expected as u16),
        );
        let error = app.submit_calculation().expect_err("retired session");
        assert_eq!(error.code, ErrorCode::ConnectionClosed);
        assert_eq!(app.state(), &FormState::Disconnected(error));
        assert_no_result(&app, "Connection failed [0x4002] - reconnect");
        let service = server.join().expect("attached task").expect("backend task");
        executor.shutdown().expect("Moirai shutdown");
        service.ledger().verify_chain().expect("audit consistency");
        assert_eq!(
            service
                .ledger()
                .records()
                .iter()
                .map(|record| (record.sequence_id, record.outcome))
                .collect::<Vec<_>>(),
            [(1, None), (2, None), (3, None)]
        );
    }
}
