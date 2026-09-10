//! Format-neutral native application lifecycle over a Moirai window.

use super::{NativeSurface, WindowConfig, WindowEvent};
use crate::Framebuffer;
use std::{error::Error, fmt, io, time::Duration};

/// Result of one application event-batch transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NativeFlow {
    /// Keep the host alive and optionally present the current framebuffer.
    Continue {
        /// Whether the application changed pixels and needs presentation.
        repaint: bool,
    },
    /// End the application session after the current batch.
    Exit,
}

/// Application-owned state consumed by the native host loop.
pub trait NativeApplication {
    /// Typed application failure returned while applying native events.
    type Error: Error + 'static;

    /// Returns the complete frame for the next native presentation.
    fn framebuffer(&self) -> &Framebuffer;

    /// Applies one bounded native event batch and reports the next host action.
    ///
    /// The batch can be empty when the finite wait expires. Applications use
    /// that case for bounded timer or animation ticks without introducing a
    /// second event loop. A `Resized` event must update the returned framebuffer
    /// before requesting a repaint.
    ///
    /// # Errors
    /// Returns the application's typed transition error without changing host
    /// ownership of the native surface.
    fn handle_events(&mut self, events: &[WindowEvent]) -> Result<NativeFlow, Self::Error>;
}

/// Failure from either the native surface or the application transition.
#[derive(Debug)]
#[non_exhaustive]
pub enum NativeHostError<E> {
    /// The window provider rejected creation, waiting, presentation or close.
    Surface(io::Error),
    /// The application rejected a native event batch.
    Application(E),
}

trait NativeSurfaceDriver {
    fn wait_events(&mut self, timeout: Duration) -> io::Result<Vec<WindowEvent>>;

    fn present(&mut self, framebuffer: &Framebuffer) -> io::Result<()>;

    fn close(&mut self) -> io::Result<()>;
}

impl NativeSurfaceDriver for NativeSurface {
    fn wait_events(&mut self, timeout: Duration) -> io::Result<Vec<WindowEvent>> {
        NativeSurface::wait_events(self, timeout)
    }

    fn present(&mut self, framebuffer: &Framebuffer) -> io::Result<()> {
        NativeSurface::present(self, framebuffer)
    }

    fn close(&mut self) -> io::Result<()> {
        NativeSurface::close(self)
    }
}

impl<E> fmt::Display for NativeHostError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Surface(error) => write!(formatter, "native surface error: {error}"),
            Self::Application(error) => write!(formatter, "native application error: {error}"),
        }
    }
}

impl<E> Error for NativeHostError<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Surface(error) => Some(error),
            Self::Application(error) => Some(error),
        }
    }
}

/// Runs a format-neutral application over a thread-owned native surface.
///
/// The host owns only window creation, finite waiting, retained-frame
/// presentation and terminal-window cleanup. The application owns all state,
/// event policy and format-specific interpretation. This keeps DICOM and
/// other domain protocols outside the platform crate.
///
/// # Errors
/// Returns [`NativeHostError::Surface`] when the provider rejects an operation
/// or [`NativeHostError::Application`] when the application rejects an event
/// batch.
pub fn run_native_application<A>(
    config: &WindowConfig,
    application: A,
    wait: Duration,
) -> Result<(), NativeHostError<A::Error>>
where
    A: NativeApplication,
{
    let surface = NativeSurface::new(config).map_err(NativeHostError::Surface)?;
    run_application_loop(surface, application, wait)
}

fn run_application_loop<S, A>(
    mut surface: S,
    application: A,
    wait: Duration,
) -> Result<(), NativeHostError<A::Error>>
where
    S: NativeSurfaceDriver,
    A: NativeApplication,
{
    let mut application = application;
    surface
        .present(application.framebuffer())
        .map_err(NativeHostError::Surface)?;

    loop {
        let events = surface
            .wait_events(wait)
            .map_err(NativeHostError::Surface)?;
        let destroyed = events.iter().any(is_destroyed);
        let terminal = events.iter().any(is_terminal);
        let flow = application
            .handle_events(&events)
            .map_err(NativeHostError::Application)?;

        if destroyed {
            return Ok(());
        }
        if terminal || matches!(flow, NativeFlow::Exit) {
            surface.close().map_err(NativeHostError::Surface)?;
            return Ok(());
        }
        if matches!(flow, NativeFlow::Continue { repaint: true }) {
            surface
                .present(application.framebuffer())
                .map_err(NativeHostError::Surface)?;
        }
    }
}

fn is_destroyed(event: &WindowEvent) -> bool {
    matches!(event, WindowEvent::Destroyed)
}

fn is_terminal(event: &WindowEvent) -> bool {
    matches!(event, WindowEvent::CloseRequested | WindowEvent::Destroyed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::{MAX_WAIT_MILLISECONDS, WindowVisibility};
    use std::collections::VecDeque;
    use std::io;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Debug)]
    struct ProbeError;

    impl fmt::Display for ProbeError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("probe application rejected the event batch")
        }
    }

    impl Error for ProbeError {}

    #[derive(Debug, Default)]
    struct ProbeObservation {
        calls: usize,
        empty_batches: usize,
        saw_resize: bool,
        repaint_count: usize,
    }

    struct ProbeApplication {
        framebuffer: Framebuffer,
        observation: Arc<Mutex<ProbeObservation>>,
        reject: bool,
    }

    impl ProbeApplication {
        fn new(reject: bool) -> Self {
            Self {
                framebuffer: Framebuffer::new(320, 240).expect("bounded probe framebuffer"),
                observation: Arc::new(Mutex::new(ProbeObservation::default())),
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

    struct PresentedFrame {
        width: u32,
        height: u32,
        pixels: Vec<u32>,
    }

    #[derive(Clone)]
    struct RecordingTrace {
        presentations: Arc<Mutex<Vec<PresentedFrame>>>,
        close_calls: Arc<AtomicUsize>,
        destroyed: Arc<AtomicBool>,
        drops: Arc<AtomicUsize>,
    }

    struct RecordingSurface {
        events: VecDeque<Vec<WindowEvent>>,
        trace: RecordingTrace,
    }

    impl RecordingSurface {
        fn new(events: impl IntoIterator<Item = Vec<WindowEvent>>) -> (Self, RecordingTrace) {
            let trace = RecordingTrace {
                presentations: Arc::new(Mutex::new(Vec::new())),
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

    impl NativeSurfaceDriver for RecordingSurface {
        fn wait_events(&mut self, _timeout: Duration) -> io::Result<Vec<WindowEvent>> {
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
                });
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

    struct PresentationApplication {
        framebuffer: Framebuffer,
        resized_frame: Option<Framebuffer>,
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
    fn terminal_event_classification_is_explicit() {
        let close = [WindowEvent::CloseRequested];
        let destroyed = [WindowEvent::Destroyed];
        let input = [WindowEvent::FocusGained];
        assert!(close.iter().any(is_terminal));
        assert!(destroyed.iter().any(is_destroyed));
        assert!(!input.iter().any(is_terminal));
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

    #[test]
    fn generic_host_returns_after_destroyed_surface_without_reclosing() {
        let (surface, trace) = RecordingSurface::new([vec![WindowEvent::Destroyed]]);
        run_application_loop(surface, ProbeApplication::new(false), Duration::ZERO)
            .expect("destroyed surface exits host");
        assert_eq!(
            trace
                .presentations
                .lock()
                .expect("presentation trace lock remains healthy")
                .len(),
            1
        );
        assert_eq!(trace.close_calls.load(Ordering::SeqCst), 0);
        assert!(trace.destroyed.load(Ordering::SeqCst));
        assert_eq!(trace.drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn generic_host_drops_surface_after_application_error() {
        let (surface, trace) = RecordingSurface::new([vec![WindowEvent::FocusGained]]);
        let error = run_application_loop(surface, ProbeApplication::new(true), Duration::ZERO)
            .expect_err("application error");
        assert!(matches!(error, NativeHostError::Application(ProbeError)));
        assert_eq!(trace.close_calls.load(Ordering::SeqCst), 0);
        assert!(trace.destroyed.load(Ordering::SeqCst));
        assert_eq!(trace.drops.load(Ordering::SeqCst), 1);
    }

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
        assert!(matches!(error, NativeHostError::Application(ProbeError)));
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
            Duration::from_millis(u64::from(MAX_WAIT_MILLISECONDS) + 1),
        )
        .expect_err("bounded wait error");
        assert!(matches!(error, NativeHostError::Surface(_)));
    }

    #[test]
    fn host_error_display_retains_source_chain() {
        let surface = NativeHostError::<ProbeError>::Surface(io::Error::other("surface failure"));
        assert_eq!(surface.to_string(), "native surface error: surface failure");
        let application = NativeHostError::Application(ProbeError);
        assert_eq!(
            application.to_string(),
            "native application error: probe application rejected the event batch"
        );
    }
}
