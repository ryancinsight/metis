//! End-to-end presentation and backend exchange on a bounded memory transport.
use metis_backend::{BackendService, clinical::SafetyEnvelope};
use metis_frontend::FrontendApp;
use metis_ipc::{server::IpcServer, transport::MemoryTransport};
use moirai_core::TaskSpawner;
use moirai_executor::ExecutorBuilder;

#[test]
fn form_input_changes_backend_result_and_audit() {
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
            while server.step(&mut service).expect("session step") {}
            service
        })
        .expect("admit backend task");
    let mut app = FrontendApp::new(front, 800, 600).expect("presentation");
    app.init(1234, [1; 16]).expect("handshake");
    for (weight, expected) in [(60.0, 0.36_f64), (80.0, 0.48_f64)] {
        app.set_inputs("demo", weight, 2.0, 0.2);
        app.submit_calculation().expect("submission");
        let result = app.last_response.as_ref().expect("backend result");
        // Four arithmetic operations and input rounding: gamma(6) bounds relative error.
        let bound = 6.0 * f64::EPSILON / (1.0 - 6.0 * f64::EPSILON);
        assert!((result.rate_ml_hr - expected).abs() <= expected * bound);
    }
    drop(app);
    let service = server.join().expect("attached task").expect("backend task");
    executor.shutdown().expect("Moirai shutdown");
    service.ledger().verify_chain().expect("audit consistency");
    assert_eq!(service.ledger().records().len(), 3);
}
