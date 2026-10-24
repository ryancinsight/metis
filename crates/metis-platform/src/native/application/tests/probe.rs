//! Probe infrastructure for native host application tests.

use crate::Framebuffer;
use crate::native::WindowEvent;
use crate::native::application::{NativeApplication, NativeFlow};
use std::error::Error;
use std::fmt;

/// Error returned by the probe application when it rejects an event batch.
#[derive(Debug)]
pub struct ProbeError;

impl fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("probe application rejected the event batch")
    }
}

impl Error for ProbeError {}

/// Records observations about the host's interaction with the probe application.
#[derive(Debug, Default)]
pub struct ProbeObservation {
    /// Number of times the host called `handle_events`.
    pub calls: usize,
    /// Number of empty event batches received.
    pub empty_batches: usize,
    /// Whether a resize event with specific dimensions was seen.
    pub saw_resize: bool,
    /// Number of repaints requested.
    pub repaint_count: usize,
}

/// A probe application that optionally rejects event batches.
pub struct ProbeApplication {
    pub framebuffer: Framebuffer,
    pub observation: std::sync::Arc<std::sync::Mutex<ProbeObservation>>,
    pub reject: bool,
}

impl ProbeApplication {
    /// Creates a new probe application.
    pub fn new(reject: bool) -> Self {
        Self {
            framebuffer: Framebuffer::new(320, 240).expect("bounded probe framebuffer"),
            observation: std::sync::Arc::new(std::sync::Mutex::new(ProbeObservation::default())),
            reject,
        }
    }
}

impl NativeApplication for ProbeApplication {
    type Error = ProbeError;

    fn framebuffer(&self) -> &Framebuffer {
        &self.framebuffer
    }

    fn handle_events(&mut self, events: &[WindowEvent]) -> Result<NativeFlow, Self::Error> {
        let mut observation = self
            .observation
            .lock()
            .expect("probe observation lock remains healthy");
        observation.calls += 1;
        if self.reject {
            return Err(ProbeError);
        }
        if events.is_empty() {
            observation.empty_batches += 1;
            return Ok(NativeFlow::Exit);
        }
        if events.iter().any(|event| {
            matches!(
                event,
                WindowEvent::Resized {
                    width,
                    height
                } if *width == 320 && *height == 240
            )
        }) {
            observation.saw_resize = true;
            observation.repaint_count += 1;
            return Ok(NativeFlow::Continue { repaint: true });
        }
        Ok(NativeFlow::Continue { repaint: false })
    }
}
