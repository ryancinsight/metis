//! Tests for destroyed surface and application errors.

use crate::native::WindowEvent;
use crate::native::application::run_application_loop;
use crate::native::application::tests::probe::{ProbeApplication, ProbeError};
use crate::native::application::tests::recording_surface::RecordingSurface;

#[test]
fn generic_host_returns_after_destroyed_surface_without_reclosing() {
    let (surface, trace) = RecordingSurface::new([vec![WindowEvent::Destroyed]]);
    run_application_loop(
        surface,
        ProbeApplication::new(false),
        std::time::Duration::ZERO,
    )
    .expect("destroyed surface exits host");
    assert_eq!(
        trace
            .presentations
            .lock()
            .expect("presentation trace lock remains healthy")
            .len(),
        1
    );
    assert_eq!(
        trace.close_calls.load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert!(trace.destroyed.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(trace.drops.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn generic_host_drops_surface_after_application_error() {
    let (surface, trace) = RecordingSurface::new([vec![WindowEvent::FocusGained]]);
    let error = run_application_loop(
        surface,
        ProbeApplication::new(true),
        std::time::Duration::ZERO,
    )
    .expect_err("application error");
    assert!(matches!(
        error,
        crate::native::application::NativeHostError::Application(ProbeError)
    ));
    assert_eq!(
        trace.close_calls.load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert!(trace.destroyed.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(trace.drops.load(std::sync::atomic::Ordering::SeqCst), 1);
}
