//! Presentation application for host tests.

use crate::Framebuffer;
use crate::native::application::{NativeApplication, NativeFlow};
use crate::native::application::tests::probe::ProbeError;
use crate::native::WindowEvent;

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