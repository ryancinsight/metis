//! Generate a bounded capture from the real Windows native host.

#[cfg(windows)]
#[path = "support/framebuffer.rs"]
mod framebuffer_artifacts;

#[cfg(windows)]
#[path = "support/native_capture.rs"]
mod native_capture_support;

#[cfg(windows)]
mod capture {
    use super::framebuffer_artifacts;
    use super::native_capture_support;
    use metis_core::error::{ErrorCode, MetisError};
    use metis_platform::native::{
        NativeApplication, NativeFlow, WindowConfig, WindowEvent, WindowVisibility,
        run_native_application,
    };
    use metis_platform::{Color, Framebuffer, Rect, draw_rect_outline, draw_text, fill_rect};
    use std::error::Error;
    use std::io;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    const FRAME_WIDTH: u32 = 320;
    const FRAME_HEIGHT: u32 = 180;
    const MAX_EVENT_BATCHES: usize = 16;

    #[derive(Debug)]
    struct Presentation {
        phase: &'static str,
        frame: Framebuffer,
    }

    #[derive(Debug)]
    struct EventBatch {
        index: usize,
        events: Vec<String>,
        action: &'static str,
        repaint: bool,
    }

    #[derive(Debug)]
    struct HostTrace {
        source_revision: String,
        event_batches: Vec<EventBatch>,
        presentations: Vec<Presentation>,
    }

    struct CaptureApplication {
        frame: Framebuffer,
        trace: Arc<Mutex<HostTrace>>,
        saw_resize: bool,
        batches: usize,
    }

    pub fn run() -> Result<(), Box<dyn Error>> {
        let (output, source_revision) =
            native_capture_support::parse_options(std::env::args_os().skip(1))?;
        std::fs::create_dir_all(&output)?;
        let trace = Arc::new(Mutex::new(HostTrace::new(source_revision)));
        let application = CaptureApplication::new(Arc::clone(&trace))?;
        let config = WindowConfig::with_visibility(
            "Metis native host capture",
            FRAME_WIDTH,
            FRAME_HEIGHT,
            WindowVisibility::Hidden,
        )?;
        run_native_application(&config, application, Duration::ZERO)?;

        let trace = Arc::try_unwrap(trace)
            .map_err(|_| io::Error::other("native host capture trace still has an owner"))?
            .into_inner()
            .map_err(|_| io::Error::other("native host capture trace lock is poisoned"))?;
        trace.validate()?;
        write_artifacts(&output, &trace)?;
        println!(
            "Captured {} presentations from the hidden Moirai NativeSurface into {}",
            trace.presentations.len(),
            output.display()
        );
        Ok(())
    }

    impl CaptureApplication {
        fn new(trace: Arc<Mutex<HostTrace>>) -> Result<Self, MetisError> {
            Ok(Self {
                frame: render_frame(FRAME_WIDTH, FRAME_HEIGHT, Color::BLUE, "HOST READY")?,
                trace,
                saw_resize: false,
                batches: 0,
            })
        }

        fn record_batch(&self, events: &[WindowEvent], action: &'static str, repaint: bool) {
            let mut trace = self
                .trace
                .lock()
                .expect("invariant: native host capture trace is thread-owned");
            let index = trace.event_batches.len();
            trace.event_batches.push(EventBatch {
                index,
                events: events.iter().map(|event| format!("{event:?}")).collect(),
                action,
                repaint,
            });
        }
    }

    impl NativeApplication for CaptureApplication {
        type Error = MetisError;

        fn framebuffer(&self) -> &Framebuffer {
            let mut trace = self
                .trace
                .lock()
                .expect("invariant: native host capture trace is thread-owned");
            let phase = match trace.presentations.len() {
                0 => "initial",
                1 => "repaint",
                _ => "unexpected",
            };
            trace.presentations.push(Presentation {
                phase,
                frame: self.frame.clone(),
            });
            &self.frame
        }

        fn handle_events(&mut self, events: &[WindowEvent]) -> Result<NativeFlow, Self::Error> {
            self.batches = self.batches.checked_add(1).ok_or_else(|| {
                MetisError::ui(
                    ErrorCode::LayoutOverflow,
                    "native host capture event-batch counter overflowed",
                )
            })?;
            if events
                .iter()
                .any(|event| matches!(event, WindowEvent::CloseRequested | WindowEvent::Destroyed))
            {
                self.record_batch(events, "exit", false);
                return Ok(NativeFlow::Exit);
            }
            if let Some((width, height)) = events.iter().find_map(|event| match event {
                WindowEvent::Resized { width, height } => Some((*width, *height)),
                _ => None,
            }) {
                self.frame = render_frame(width, height, Color::GREEN, "HOST RESIZED")?;
                self.saw_resize = true;
                self.record_batch(events, "replace framebuffer", true);
                return Ok(NativeFlow::Continue { repaint: true });
            }
            if self.saw_resize && events.is_empty() {
                self.record_batch(events, "exit", false);
                return Ok(NativeFlow::Exit);
            }
            if self.batches >= MAX_EVENT_BATCHES {
                return Err(MetisError::ui(
                    ErrorCode::Timeout,
                    "native host did not deliver readiness and a finite empty tick",
                ));
            }
            self.record_batch(events, "continue", false);
            Ok(NativeFlow::Continue { repaint: false })
        }
    }

    impl HostTrace {
        fn new(source_revision: String) -> Self {
            Self {
                source_revision,
                event_batches: Vec::new(),
                presentations: Vec::new(),
            }
        }

        fn validate(&self) -> io::Result<()> {
            if self.source_revision.is_empty() || self.source_revision.len() > 128 {
                return Err(invalid_data(
                    "source revision is empty or exceeds the trace bound",
                ));
            }
            if self.presentations.len() != 2 {
                return Err(invalid_data(
                    "real native host did not present exactly two frames",
                ));
            }
            let initial = self
                .presentations
                .first()
                .ok_or_else(|| invalid_data("initial presentation is missing"))?;
            if initial.phase != "initial"
                || initial.frame.width() != FRAME_WIDTH
                || initial.frame.height() != FRAME_HEIGHT
            {
                return Err(invalid_data(
                    "initial native frame differs from the configured surface",
                ));
            }
            let resized_index = self
                .event_batches
                .iter()
                .position(|batch| batch.action == "replace framebuffer" && batch.repaint)
                .ok_or_else(|| invalid_data("resize event did not request a repaint"))?;
            let exit_index = self
                .event_batches
                .iter()
                .position(|batch| batch.action == "exit" && batch.events.is_empty())
                .ok_or_else(|| {
                    invalid_data("real native host did not deliver an empty terminal tick")
                })?;
            if exit_index <= resized_index {
                return Err(invalid_data(
                    "terminal tick preceded the resized presentation",
                ));
            }
            let resized = self
                .presentations
                .get(1)
                .ok_or_else(|| invalid_data("resized presentation is missing"))?;
            if resized.phase != "repaint"
                || resized.frame.width() == 0
                || resized.frame.height() == 0
                || resized.frame.pixels().is_empty()
            {
                return Err(invalid_data("resized native frame is empty or mislabeled"));
            }
            if pixel_checksum(initial.frame.pixels()) == pixel_checksum(resized.frame.pixels()) {
                return Err(invalid_data(
                    "resize repaint did not change the presented pixels",
                ));
            }
            for presentation in &self.presentations {
                let count = u64::from(presentation.frame.width())
                    .checked_mul(u64::from(presentation.frame.height()))
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or_else(|| invalid_data("native frame pixel count overflowed"))?;
                if presentation.frame.pixels().len() != count {
                    return Err(invalid_data(
                        "native frame pixel count differs from dimensions",
                    ));
                }
            }
            Ok(())
        }

        fn json(&self) -> String {
            let batches = self
                .event_batches
                .iter()
                .map(EventBatch::json)
                .collect::<Vec<_>>()
                .join(",\n");
            let presentations = self
                .presentations
                .iter()
                .map(Presentation::json)
                .collect::<Vec<_>>()
                .join(",\n");
            format!(
                "{{\n  \"schema\": 2,\n  \"source_revision\": {},\n  \"example\": \"native_host_capture\",\n  \"surface\": \"real hidden Moirai NativeSurface\",\n  \"host_result\": \"ok\",\n  \"event_batches\": [\n{}\n  ],\n  \"presentations\": [\n{}\n  ]\n}}\n",
                native_capture_support::json_string(&self.source_revision),
                batches,
                presentations
            )
        }
    }

    impl EventBatch {
        fn json(&self) -> String {
            let events = self
                .events
                .iter()
                .map(|event| native_capture_support::json_string(event))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "    {{\"index\": {}, \"events\": [{}], \"application_action\": {}, \"repaint\": {}}}",
                self.index,
                events,
                native_capture_support::json_string(self.action),
                self.repaint
            )
        }
    }

    impl Presentation {
        fn json(&self) -> String {
            let samples = sample_pixels(&self.frame)
                .into_iter()
                .map(|pixel| native_capture_support::json_string(&format!("0x{pixel:08x}")))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "    {{\"phase\": {}, \"width\": {}, \"height\": {}, \"pixel_count\": {}, \"pixel_checksum_fnv1a64\": {}, \"sample_argb8888\": [{}]}}",
                native_capture_support::json_string(self.phase),
                self.frame.width(),
                self.frame.height(),
                self.frame.pixels().len(),
                native_capture_support::json_string(&format!(
                    "0x{:016x}",
                    pixel_checksum(self.frame.pixels())
                )),
                samples
            )
        }
    }

    fn render_frame(
        width: u32,
        height: u32,
        accent: Color,
        label: &str,
    ) -> Result<Framebuffer, MetisError> {
        let mut frame = Framebuffer::new(width, height)?;
        let width = i32::try_from(width).map_err(|_| {
            MetisError::ui(
                ErrorCode::LayoutOverflow,
                "native capture width exceeds coordinates",
            )
        })?;
        let height = i32::try_from(height).map_err(|_| {
            MetisError::ui(
                ErrorCode::LayoutOverflow,
                "native capture height exceeds coordinates",
            )
        })?;
        frame.clear(Color::DARK_BLUE);
        fill_rect(
            &mut frame,
            Rect::new(12, 12, width.saturating_sub(24), height.saturating_sub(24)),
            Color::rgba(15, 23, 42, 255),
        );
        draw_rect_outline(
            &mut frame,
            Rect::new(12, 12, width.saturating_sub(24), height.saturating_sub(24)),
            2,
            accent,
        );
        draw_text(&mut frame, 24, 32, "METIS NATIVE HOST", Color::WHITE, 2);
        draw_text(&mut frame, 24, 76, label, accent, 2);
        draw_text(
            &mut frame,
            24,
            height.saturating_sub(34),
            "FORMAT-NEUTRAL FRAME",
            Color::LIGHT_GRAY,
            1,
        );
        Ok(frame)
    }

    fn write_artifacts(output: &Path, trace: &HostTrace) -> Result<(), Box<dyn Error>> {
        let frame = trace
            .presentations
            .last()
            .ok_or_else(|| invalid_data("capture has no final frame"))?;
        let bitmap = framebuffer_artifacts::bmp_bytes(&frame.frame)?;
        let svg = framebuffer_artifacts::svg_text(&frame.frame)?;
        let bitmap_path = output.join("native-host-frame.bmp");
        let svg_path = output.join("native-host-frame.svg");
        let trace_path = output.join("native-host-trace.json");
        std::fs::write(&bitmap_path, &bitmap)?;
        std::fs::write(&svg_path, &svg)?;
        let trace_json = trace.json();
        std::fs::write(&trace_path, &trace_json)?;

        let stored_bitmap = std::fs::read(&bitmap_path)?;
        let (width, height, pixels) = native_capture_support::decode_bmp(&stored_bitmap)?;
        if (width, height) != (frame.frame.width(), frame.frame.height())
            || pixels != frame.frame.pixels()
        {
            return Err(
                invalid_data("native host bitmap differs from the presented framebuffer").into(),
            );
        }
        if std::fs::read_to_string(&svg_path)? != svg {
            return Err(
                invalid_data("native host SVG differs from the generated framebuffer").into(),
            );
        }
        if std::fs::read_to_string(&trace_path)? != trace_json {
            return Err(invalid_data(
                "native host trace differs from the generated execution record",
            )
            .into());
        }
        Ok(())
    }

    fn sample_pixels(frame: &Framebuffer) -> Vec<u32> {
        let mut indices = vec![
            0,
            frame.pixels().len() / 2,
            frame.pixels().len().saturating_sub(1),
        ];
        if let Ok(width) = usize::try_from(frame.width())
            && let Some(index) = 12_usize
                .checked_mul(width)
                .and_then(|row| row.checked_add(12))
        {
            indices.push(index);
        }
        indices.sort_unstable();
        indices.dedup();
        indices
            .into_iter()
            .filter_map(|index| frame.pixels().get(index).copied())
            .collect()
    }

    /// FNV-1a is a compact deterministic identity for the artifact trace; the
    /// full bitmap round-trip below remains the value-semantic check.
    fn pixel_checksum(pixels: &[u32]) -> u64 {
        pixels
            .iter()
            .flat_map(|pixel| pixel.to_le_bytes())
            .fold(14_695_981_039_346_656_037_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(1_099_511_628_211_u64)
            })
    }

    fn invalid_data(message: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, message)
    }
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    capture::run()
}

#[cfg(not(windows))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "native_host_capture requires a Windows native host",
    )
    .into())
}
