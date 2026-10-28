//! Readiness and tick delivery tests for host.

use crate::native::application::run_native_application;
use crate::native::application::tests::probe::{ProbeApplication, ProbeError};
use crate::native::{WindowConfig, WindowVisibility};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn host_delivers_readiness_and_empty_tick_before_exit() {
    let config = WindowConfig::with_visibility(
        "Metis application host test",
        320,
        240,
        WindowVisibility::Hidden,
    )
    .expect("bounded native configuration");
    let application = ProbeApplication::new(false);
    let observation = Arc::clone(&application.observation);
    run_native_application(&config, application, Duration::ZERO).expect("host loop");
    let observation = observation
        .lock()
        .expect("probe observation lock remains healthy");
    assert!(observation.calls >= 2);
    assert!(observation.empty_batches >= 1);
    assert!(observation.saw_resize);
    assert!(observation.repaint_count >= 1);
}

#[test]
fn host_preserves_typed_application_errors() {
    let config = WindowConfig::with_visibility(
        "Metis application error test",
        320,
        240,
        WindowVisibility::Hidden,
    )
    .expect("bounded native configuration");
    let error = run_native_application(&config, ProbeApplication::new(true), Duration::ZERO)
        .expect_err("application error");
    assert!(matches!(
        error,
        crate::native::application::NativeHostError::Application(ProbeError)
    ));
}

#[test]
fn host_preserves_provider_wait_error_typed() {
    let config = WindowConfig::with_visibility(
        "Metis application surface error test",
        320,
        240,
        WindowVisibility::Hidden,
    )
    .expect("bounded native configuration");
    let error = run_native_application(
        &config,
        ProbeApplication::new(false),
        Duration::from_millis(u64::from(crate::native::MAX_WAIT_MILLISECONDS) + 1),
    )
    .expect_err("bounded wait error");
    assert!(matches!(
        error,
        crate::native::application::NativeHostError::Surface(_)
    ));
}
