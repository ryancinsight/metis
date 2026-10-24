//! Presentation application for host tests.

use crate::Framebuffer;
use crate::native::WindowEvent;
use crate::native::application::tests::probe::ProbeError;
use crate::native::application::tests::recording_surface::RecordingSurface;
use crate::native::application::{NativeApplication, NativeFlow, run_application_loop};
use std::sync::atomic::Ordering;
use std::time::Duration;

/// A presentation application that handles resize and close events.
pub struct PresentationApplication {
    pub framebuffer: Framebuffer,
    pub resized_frame: Option<Framebuffer>,
}

impl NativeApplication for PresentationApplication {
    type Error = ProbeError;

    fn framebuffer(&self) -> &Framebuffer {
        &self.framebuffer
    }

    fn handle_events(&mut self, events: &[WindowEvent]) -> Result<NativeFlow, Self::Error> {
        if events
            .iter()
            .any(|event| matches!(event, WindowEvent::CloseRequested))
        {
            return Ok(NativeFlow::Exit);
        }
        if events.iter().any(|event| {
            matches!(
                event,
                WindowEvent::Resized {
                    width: 3,
                    height: 2
                }
            )
        }) {
            self.framebuffer = self.resized_frame.take().expect("single resize fixture");
            return Ok(NativeFlow::Continue { repaint: true });
        }
        Ok(NativeFlow::Continue { repaint: false })
    }
}

#[test]
fn generic_host_presents_initial_and_resized_frames_then_closes() {
    let mut initial = Framebuffer::new(2, 2).expect("bounded initial framebuffer");
    initial.clear(crate::Color::BLUE);
    let initial_pixels = initial.pixels().to_vec();
    let mut resized = Framebuffer::new(3, 2).expect("bounded resized framebuffer");
    resized.clear(crate::Color::GREEN);
    let resized_pixels = resized.pixels().to_vec();
    let application = PresentationApplication {
        framebuffer: initial,
        resized_frame: Some(resized),
    };
    let (surface, trace) = RecordingSurface::new([
        vec![WindowEvent::Resized {
            width: 3,
            height: 2,
        }],
        vec![WindowEvent::CloseRequested],
    ]);

    run_application_loop(surface, application, Duration::ZERO).expect("recording host loop");

    let presentations = trace
        .presentations
        .lock()
        .expect("presentation trace lock remains healthy");
    assert_eq!(presentations.len(), 2);
    assert_eq!((presentations[0].width, presentations[0].height), (2, 2));
    assert_eq!(presentations[0].pixels, initial_pixels);
    assert_eq!((presentations[1].width, presentations[1].height), (3, 2));
    assert_eq!(presentations[1].pixels, resized_pixels);
    assert_eq!(trace.close_calls.load(Ordering::SeqCst), 1);
    assert!(trace.destroyed.load(Ordering::SeqCst));
    assert_eq!(trace.drops.load(Ordering::SeqCst), 1);
}
