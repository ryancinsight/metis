use super::*;
use crate::native::{
    AccessibilityNode, AccessibilityRole, MAX_WAIT_MILLISECONDS, WindowVisibility,
};
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
    region: Option<Rect>,
}

#[derive(Clone)]
struct RecordingTrace {
    presentations: Arc<Mutex<Vec<PresentedFrame>>>,
    /// Every timeout the host asked the provider to wait, in call order.
    waits: Arc<Mutex<Vec<Duration>>>,
    accessibility_updates: Arc<AtomicUsize>,
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

impl NativeSurfaceDriver for RecordingSurface {
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

    fn update_accessibility(&mut self, _tree: AccessibilityTree) -> io::Result<()> {
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

/// A validated tree a host can push, as a native provider receives one.
fn accessibility_tree() -> AccessibilityTree {
    let mut button = AccessibilityNode::new(2, AccessibilityRole::Button, "Submit")
        .expect("accessibility button");
    button.set_focusable(true);
    let mut root = AccessibilityNode::new(1, AccessibilityRole::Application, "Metis")
        .expect("accessibility root");
    root.set_children(vec![2]).expect("root children");
    AccessibilityTree::from_nodes(1, 2, vec![root, button]).expect("validated tree")
}

/// Projects one tree, then reports no semantic change, and counts the
/// projections so a push the host did not need is visible.
struct OnceAccessibilityApplication {
    framebuffer: Framebuffer,
    projections: Arc<AtomicUsize>,
    changed: bool,
}

impl NativeApplication for OnceAccessibilityApplication {
    type Error = ProbeError;

    fn framebuffer(&self) -> &Framebuffer {
        &self.framebuffer
    }

    fn accessibility_tree(&self) -> Result<Option<AccessibilityTree>, Self::Error> {
        self.projections.fetch_add(1, Ordering::SeqCst);
        Ok(Some(accessibility_tree()))
    }

    fn take_accessibility_damage(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    fn handle_events(&mut self, events: &[WindowEvent]) -> Result<NativeFlow, Self::Error> {
        if events
            .iter()
            .any(|event| matches!(event, WindowEvent::CloseRequested))
        {
            return Ok(NativeFlow::Exit);
        }
        Ok(NativeFlow::Continue { repaint: false })
    }
}

#[test]
fn an_idle_batch_projects_and_pushes_no_accessibility_tree() {
    let projections = Arc::new(AtomicUsize::new(0));
    let application = OnceAccessibilityApplication {
        framebuffer: Framebuffer::new(2, 2).expect("bounded framebuffer"),
        projections: Arc::clone(&projections),
        changed: true,
    };
    let focus = || vec![WindowEvent::FocusGained];
    let (surface, trace) =
        RecordingSurface::new([focus(), focus(), focus(), vec![WindowEvent::CloseRequested]]);

    run_application_loop(surface, application, Duration::ZERO).expect("recording host loop");

    // One semantic change over three batches: the first projects, the rest do
    // not, and the provider receives exactly one tree.
    assert_eq!(projections.load(Ordering::SeqCst), 1);
    assert_eq!(trace.accessibility_updates.load(Ordering::SeqCst), 1);
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

/// Reports a scripted damage for each repaint it requests.
struct DamageApplication {
    framebuffer: Framebuffer,
    damages: VecDeque<Damage>,
    pending: Damage,
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

/// Reports a tick to the host and counts the batches the host delivers, so a
/// wait the application did not ask for is visible.
struct TickingApplication {
    framebuffer: Framebuffer,
    tick: Option<Duration>,
    batches: Arc<AtomicUsize>,
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
fn recorded_waits(wait: Duration, tick: Option<Duration>) -> (Vec<Duration>, usize) {
    let batches = Arc::new(AtomicUsize::new(0));
    let application = TickingApplication {
        framebuffer: Framebuffer::new(2, 2).expect("bounded framebuffer"),
        tick,
        batches: Arc::clone(&batches),
    };
    let focus = || vec![WindowEvent::FocusGained];
    let (surface, trace) =
        RecordingSurface::new([focus(), focus(), vec![WindowEvent::CloseRequested]]);

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
