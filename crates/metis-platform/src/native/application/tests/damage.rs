//! Damage reporting tests for host.

use crate::native::WindowEvent;
use crate::native::application::tests::probe::ProbeError;
use crate::native::application::tests::recording_surface::RecordingSurface;
use crate::native::application::{NativeApplication, NativeFlow, run_application_loop};
use crate::{Damage, Framebuffer, Rect};
use std::collections::VecDeque;
use std::time::Duration;

/// Reports a scripted damage for each repaint it requests.
pub struct DamageApplication {
    pub framebuffer: Framebuffer,
    pub damages: VecDeque<Damage>,
    pub pending: Damage,
}

impl NativeApplication for DamageApplication {
    type Error = ProbeError;

    fn framebuffer(&self) -> &Framebuffer {
        &self.framebuffer
    }

    fn take_damage(&mut self) -> Damage {
        std::mem::replace(&mut self.pending, Damage::Unchanged)
    }

    fn handle_events(&mut self, events: &[WindowEvent]) -> Result<NativeFlow, Self::Error> {
        if events
            .iter()
            .any(|event| matches!(event, WindowEvent::CloseRequested))
        {
            return Ok(NativeFlow::Exit);
        }
        self.pending = self
            .pending
            .merge(self.damages.pop_front().ok_or(ProbeError)?);
        Ok(NativeFlow::Continue { repaint: true })
    }
}

#[test]
fn generic_host_presents_exactly_the_reported_damage() {
    let region = Rect::new(1, 1, 2, 1);
    let application = DamageApplication {
        framebuffer: Framebuffer::new(4, 3).expect("bounded framebuffer"),
        damages: [Damage::Region(region), Damage::Unchanged, Damage::Full].into(),
        // Damage from before the first presentation is covered by it.
        pending: Damage::Region(Rect::new(0, 0, 1, 1)),
    };
    let focus = || vec![WindowEvent::FocusGained];
    let (surface, trace) =
        RecordingSurface::new([focus(), focus(), focus(), vec![WindowEvent::CloseRequested]]);

    run_application_loop(surface, application, Duration::ZERO).expect("recording host loop");

    let regions: Vec<Option<Rect>> = trace
        .presentations
        .lock()
        .expect("presentation trace lock remains healthy")
        .iter()
        .map(|presented| presented.region)
        .collect();
    assert_eq!(regions, [None, Some(region), None]);
}
