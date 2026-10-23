//! Ticking application and wait timing tests.

use crate::native::application::{NativeApplication, NativeFlow, run_application_loop};
use crate::{Framebuffer};
use crate::native::WindowEvent;
use crate::native::application::tests::recording_surface::RecordingSurface;
use crate::native::application::tests::probe::ProbeError;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Reports a tick to the host and counts the batches the host delivers.
pub struct TickingApplication {
    pub framebuffer: Framebuffer,
    pub tick: Option<Duration>,
    pub batches: Arc<AtomicUsize>,
}

impl NativeApplication for TickingApplication {
    type Error = ProbeError;

    fn framebuffer(&self) -> &Framebuffer {
        &self.framebuffer
    }

    fn next_tick(&self) -> Option<Duration> {
        self.tick
    }

    fn handle_events(&mut self, events: &[WindowEvent]) -> Result<NativeFlow, Self::Error> {
        if events
            .iter()
            .any(|event| matches!(event, WindowEvent::CloseRequested))
        {
            return Ok(NativeFlow::Exit);
        }
        self.batches.fetch_add(1, Ordering::SeqCst);
        Ok(NativeFlow::Continue { repaint: false })
    }
}

/// Runs one host loop with the given tick and returns every wait it asked for.
pub fn recorded_waits(wait: Duration, tick: Option<Duration>) -> (Vec<Duration>, usize) {
    let batches = Arc::new(AtomicUsize::new(0));
    let application = TickingApplication {
        framebuffer: Framebuffer::new(2, 2).expect("bounded framebuffer"),
        tick,
        batches: Arc::clone(&batches),
    };
    let focus = || vec![WindowEvent::FocusGained];
    let (surface, trace) = RecordingSurface::new([
        focus(),
        focus(),
        vec![WindowEvent::CloseRequested],
    ]);

    run_application_loop(surface, application, wait).expect("recording host loop");

    let waits = trace
        .waits
        .lock()
        .expect("wait trace lock remains healthy")
        .clone();
    (waits, batches.load(Ordering::SeqCst))
}

#[test]
fn a_moving_application_shortens_the_host_wait_to_its_tick() {
    let wait = Duration::from_millis(200);
    let tick = Duration::from_millis(16);

    let (waits, batches) = recorded_waits(wait, Some(tick));

    // Every batch, including the one that carries the close request, waits the
    // application's tick rather than the host's whole wait.
    assert_eq!(waits, [tick, tick, tick]);
    assert_eq!(batches, 2);
}

#[test]
fn an_idle_or_slower_application_keeps_the_whole_host_wait() {
    let wait = Duration::from_millis(200);

    // No tick: the surface sleeps for the host's wait.
    assert_eq!(recorded_waits(wait, None).0, [wait, wait, wait]);
    // A tick longer than the wait never lengthens it.
    assert_eq!(
        recorded_waits(wait, Some(Duration::from_secs(1))).0,
        [wait, wait, wait]
    );
}