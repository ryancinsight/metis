//! Captures real form transitions through the backend and production framebuffer.
use metis_backend::{BackendService, clinical::SafetyEnvelope};
use metis_core::{ErrorCode, MetisError};
use metis_frontend::{FormState, FrontendApp};
use metis_ipc::{server::IpcServer, transport::MemoryTransport};
#[path = "presentation/capture.rs"]
mod capture;
use capture::{Oracle, capture, comparator_probes};

use moirai_core::TaskSpawner;
use moirai_executor::ExecutorBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all("output")?;
    let mut actions = Vec::new();
    let (mut app, service) = session_trace(4, &mut actions, |app, actions| {
        assert_idle(app);
        assert_label(app, "status-badge", "SYSTEM READY");
        capture(app, "form", Oracle::Idle, actions)?;
        comparator_probes(app.document())?;
        initialize(app, actions)?;
        set_inputs(app, 60.0, actions)?;
        submit(app, actions)?;
        assert_result(app, 60.0, 0.36, 0.72, 2);
        capture(
            app,
            "form-success",
            Oracle::Success {
                rate: 0.36,
                drug_rate: 0.72,
                sequence: 2,
            },
            actions,
        )?;

        set_inputs(app, 80.0, actions)?;
        assert_idle(app);
        assert_label(app, "label-weight", "Weight: 80.00 kg");
        capture(app, "form-edited", Oracle::Idle, actions)?;

        set_inputs(app, 0.0, actions)?;
        submit(app, actions)?;
        let FormState::Rejected(error) = app.state() else {
            return Err("Expected backend rejection of zero weight".into());
        };
        assert_eq!(error.error_code, ErrorCode::InvalidPatientWeight as u16);
        assert_no_result(app, "Backend rejected request [0x3001]");
        capture(
            app,
            "form-rejected",
            Oracle::Rejected(ErrorCode::InvalidPatientWeight as u16),
            actions,
        )?;

        set_inputs(app, 60.0, actions)?;
        submit(app, actions)?;
        assert_result(app, 60.0, 0.36, 0.72, 4);
        capture(
            app,
            "form-corrected",
            Oracle::Success {
                rate: 0.36,
                drug_rate: 0.72,
                sequence: 4,
            },
            actions,
        )
    })?;
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
            (3, Some(ErrorCode::InvalidPatientWeight)),
            (4, None)
        ]
    );
    // session_trace joins the finite worker: the peer has actually closed.
    let error = submit(&mut app, &mut actions).expect_err("closed backend endpoint");
    assert_eq!(error.code, ErrorCode::ConnectionClosed);
    assert_eq!(app.state(), &FormState::Disconnected(error));
    assert_no_result(&app, "Connection failed [0x4002] - reconnect");
    assert_label(&app, "status-badge", "SESSION CLOSED");
    capture(
        &app,
        "form-disconnected",
        Oracle::Disconnected(ErrorCode::ConnectionClosed as u16),
        &actions,
    )?;

    let (_, recovered) = session_trace(2, &mut actions, |app, actions| {
        initialize(app, actions)?;
        set_inputs(app, 80.0, actions)?;
        submit(app, actions)?;
        assert_result(app, 80.0, 0.48, 0.96, 2);
        capture(
            app,
            "form-recovered",
            Oracle::Success {
                rate: 0.48,
                drug_rate: 0.96,
                sequence: 2,
            },
            actions,
        )
    })?;
    assert_eq!(recovered.ledger().records().len(), 2);
    Ok(())
}

enum SessionCompletion {
    Processed(Box<BackendService>),
    PeerClosed,
}

#[derive(Debug)]
struct CleanupFailure {
    primary: Box<dyn std::error::Error>,
    cleanup: Box<dyn std::error::Error>,
}

impl std::fmt::Display for CleanupFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Demonstration failed; cleanup also failed: {}",
            self.cleanup
        )
    }
}

impl std::error::Error for CleanupFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.primary.as_ref())
    }
}

fn session_trace(
    request_count: usize,
    actions: &mut Vec<String>,
    trace: impl FnOnce(
        &mut FrontendApp<MemoryTransport>,
        &mut Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>>,
) -> Result<(FrontendApp<MemoryTransport>, BackendService), Box<dyn std::error::Error>> {
    let (front, back) = MemoryTransport::pair();
    let mut executor = ExecutorBuilder::new()
        .worker_threads(1)
        .async_threads(1)
        .build()?;
    let server = executor.spawn_blocking(move || -> Result<SessionCompletion, MetisError> {
        // A public fixture key is confined to this synthetic demonstration.
        let mut service = BackendService::new([0x77; 32], SafetyEnvelope::default());
        let mut server = IpcServer::new(back);
        for _ in 0..request_count {
            if !server.step(&mut service)? {
                service.ledger().verify_chain()?;
                return Ok(SessionCompletion::PeerClosed);
            }
        }
        service.ledger().verify_chain()?;
        Ok(SessionCompletion::Processed(Box::new(service)))
    })?;
    let mut app = FrontendApp::new(front, 800, 600)?;
    actions.push(format!(
        "create session: 800x600, bounded MemoryTransport, backend exchanges={request_count}"
    ));
    let outcome = trace(&mut app, actions);
    let app = match outcome {
        Ok(()) => Ok(app),
        Err(error) => {
            // Close the real endpoint before joining a worker awaiting another request.
            drop(app);
            Err(error)
        }
    };
    let completion = (|| -> Result<SessionCompletion, Box<dyn std::error::Error>> {
        Ok(server
            .join()
            .ok_or("Backend task lost its result handle")???)
    })();
    let shutdown = executor.shutdown();
    let result = match app {
        Ok(app) => completion.and_then(|completion| match completion {
            SessionCompletion::Processed(service) => Ok((app, *service)),
            SessionCompletion::PeerClosed => Err(MetisError::transport(
                ErrorCode::ConnectionClosed,
                "Demonstration ended before its expected exchanges",
            )
            .into()),
        }),
        Err(primary) => match completion {
            // A drained worker or clean EOF is expected after deliberate endpoint close.
            Ok(SessionCompletion::Processed(_) | SessionCompletion::PeerClosed) => Err(primary),
            Err(cleanup) => {
                Err(Box::new(CleanupFailure { primary, cleanup }) as Box<dyn std::error::Error>)
            }
        },
    };
    let result: Result<_, Box<dyn std::error::Error>> = match (result, shutdown) {
        (result, Ok(())) => result,
        (Ok(_), Err(error)) => Err(error.into()),
        (Err(primary), Err(cleanup)) => Err(Box::new(CleanupFailure {
            primary,
            cleanup: Box::new(cleanup),
        })),
    };
    let (app, service) = result?;
    actions.push("join backend worker: endpoint closed".into());
    Ok((app, service))
}

fn initialize(
    app: &mut FrontendApp<MemoryTransport>,
    actions: &mut Vec<String>,
) -> Result<(), metis_ipc::client::HandshakeError> {
    actions.push("initialize: process_id=1234, principal=[1;16]".into());
    app.init(1234, [1; 16])
}

fn set_inputs(
    app: &mut FrontendApp<MemoryTransport>,
    weight: f64,
    actions: &mut Vec<String>,
) -> Result<(), MetisError> {
    actions.push(format!("set inputs: patient_id=\"demo\", weight_kg={weight}, concentration_mg_ml=2, target_dose_mcg_kg_min=0.2"));
    app.set_inputs("demo", weight, 2.0, 0.2)
}

fn submit(
    app: &mut FrontendApp<MemoryTransport>,
    actions: &mut Vec<String>,
) -> Result<(), MetisError> {
    actions.push("submit calculation".into());
    app.submit_calculation()
}

fn assert_label(app: &FrontendApp<MemoryTransport>, id: &str, expected: &str) {
    let element = app
        .document()
        .find_element_by_id(id)
        .expect("authored form label");
    assert_eq!(element.text_content(), expected, "label {id}");
}

fn assert_idle(app: &FrontendApp<MemoryTransport>) {
    assert_eq!(app.state(), &FormState::Idle);
    assert_label(app, "output-rate", "Rate: Awaiting Backend Calculation...");
    assert_label(app, "output-status", "Safety Status: Idle");
    assert_label(app, "output-signature", "Backend MAC: None");
}

fn assert_no_result(app: &FrontendApp<MemoryTransport>, status: &str) {
    assert_label(app, "output-rate", "Rate: No result");
    assert_label(app, "output-status", status);
    assert_label(app, "output-signature", "Backend MAC: No result");
}

fn assert_result(
    app: &FrontendApp<MemoryTransport>,
    weight: f64,
    rate: f64,
    drug_rate: f64,
    sequence: u64,
) {
    assert_eq!(app.inputs().patient_id, "demo");
    assert_eq!(app.inputs().weight_kg.to_bits(), weight.to_bits());
    assert_eq!(
        app.inputs().concentration_mg_ml.to_bits(),
        2.0_f64.to_bits()
    );
    assert_eq!(
        app.inputs().target_dose_mcg_kg_min.to_bits(),
        0.2_f64.to_bits()
    );
    assert_label(app, "label-patient", "Patient ID: demo");
    assert_label(app, "label-weight", &format!("Weight: {weight:.2} kg"));
    assert_label(app, "label-conc", "Drug Concentration: 2.00 mg/mL");
    assert_label(app, "label-dose", "Target Dose: 0.200 mcg/kg/min");
    let FormState::Success(result) = app.state() else {
        panic!("Expected backend calculation, got {:?}", app.state());
    };
    // Four rate operations, the concentration multiplication and input rounding
    // fit gamma(6); these analytic values do not come from the backend algorithm.
    let bound = 6.0 * f64::EPSILON / (1.0 - 6.0 * f64::EPSILON);
    assert!((result.rate_ml_hr - rate).abs() <= rate * bound);
    assert!((result.drug_rate_mg_hr - drug_rate).abs() <= drug_rate * bound);
    assert_eq!(result.audit_sequence_id, sequence);
    assert!(!result.is_pediatric);
    assert_label(
        app,
        "output-rate",
        &format!("Rate: {rate:.3} mL/hr ({drug_rate:.2} mg/hr)"),
    );
    assert_label(
        app,
        "output-status",
        &format!("Backend response (Audit Seq #{sequence})"),
    );
    assert_label(
        app,
        "output-signature",
        "Backend MAC: Present (not verified by frontend)",
    );
    assert_label(app, "status-badge", "SESSION ACTIVE");
}
