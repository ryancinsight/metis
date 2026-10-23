//! Recording surface driver for native host tests.

use crate::{Damage, Framebuffer, Rect};
use crate::native::AccessibilityTree;
use crate::native::WindowEvent;
use std::collections::VecDeque;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A frame presented by the host.
#[derive(Debug, Clone)]
pub struct PresentedFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u32>,
    pub region: Option<Rect>,
}

/// Trace of all host interactions.
#[derive(Clone)]
pub struct RecordingTrace {
    /// Every frame the host presented.
    pub presentations: Arc<Mutex<Vec<PresentedFrame>>>,
    /// Every timeout the host asked the provider to wait.
    pub waits: Arc<Mutex<Vec<Duration>>>,
    /// Number of accessibility updates pushed.
    pub accessibility_updates: Arc<AtomicUsize>,
    /// Number of close calls.
    pub close_calls: Arc<AtomicUsize>,
    /// Whether the surface was destroyed.
    pub destroyed: Arc<AtomicBool>,
    /// Number of drops.
    pub drops: Arc<AtomicUsize>,
}

/// A surface that records all host interactions.
pub struct RecordingSurface {
    events: VecDeque<Vec<WindowEvent>>,
    trace: RecordingTrace,
}

impl RecordingSurface {
    /// Creates a new recording surface with the given event batches.
    pub fn new(
        events: impl IntoIterator<Item = Vec<WindowEvent>>,
    ) -> (Self, RecordingTrace) {
        let trace = RecordingTrace {
            presentations: Arc::new(Mutex::new(Vec::new())),
            waits: Arc::new(Mutex::new(Vec::new())),
            accessibility_updates: Arc::new(AtomicUsize::new(0)),
            close_calls: Arc::new(AtomicUsize::new(0)),
            destroyed: Arc::new(AtomicBool::new(false)),
            drops: Arc::new(AtomicUsize::new(0)),
        };
        (
            Self {
                events: events.into_iter().collect(),
                trace: trace.clone(),
            },
            trace,
        )
    }
}

impl super::super::NativeSurfaceDriver for RecordingSurface {
    fn wait_events(&mut self, timeout: Duration) -> io::Result<Vec<WindowEvent>> {
        self.trace
            .waits
            .lock()
            .expect("wait trace lock remains healthy")
            .push(timeout);
        Ok(self.events.pop_front().unwrap_or_default())
    }

    fn present(&mut self, framebuffer: &Framebuffer) -> io::Result<()> {
        self.trace
            .presentations
            .lock()
            .expect("presentation trace lock remains healthy")
            .push(PresentedFrame {
                width: framebuffer.width(),
                height: framebuffer.height(),
                pixels: framebuffer.pixels().to_vec(),
                region: None,
            });
        Ok(())
    }

    fn present_region(&mut self, framebuffer: &Framebuffer, region: Rect) -> io::Result<()> {
        self.present(framebuffer)?;
        let mut presentations = self
            .trace
            .presentations
            .lock()
            .expect("presentation trace lock remains healthy");
        if let Some(last) = presentations.last_mut() {
            last.region = Some(region);
        }
        Ok(())
    }

    fn update_accessibility(&mut self, _tree: crate::native::AccessibilityTree) -> io::Result<()> {
        self.trace
            .accessibility_updates
            .fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn close(&mut self) -> io::Result<()> {
        self.trace.close_calls.fetch_add(1, Ordering::SeqCst);
        self.trace.destroyed.store(true, Ordering::SeqCst);
        Ok(())
    }
}

impl Drop for RecordingSurface {
    fn drop(&mut self) {
        self.trace.destroyed.store(true, Ordering::SeqCst);
        self.trace.drops.fetch_add(1, Ordering::SeqCst);
    }
}