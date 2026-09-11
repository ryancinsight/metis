//! Thin Python access to the Rust-owned native application host.

use crate::error::map_error;
use metis_core::error::{ErrorCode, MetisError, Result};
use pyo3::prelude::*;
use pyo3::types::PyList;

#[cfg(not(windows))]
fn unsupported_error() -> MetisError {
    MetisError::ui(
        ErrorCode::UnsupportedPlatformEvent,
        "NativeApplication is unavailable on this platform",
    )
}

#[cfg(windows)]
mod windows_host {
    use super::{ErrorCode, MetisError, Result};
    use metis_platform::Framebuffer;
    pub(super) use metis_platform::native::MAX_WAIT_MILLISECONDS;
    use metis_platform::native::{
        CompositionPhase, ModifierState, MouseButton, NativeSurface, WindowConfig, WindowEvent,
        WindowVisibility,
    };
    use pyo3::prelude::{Bound, PyResult, Python};
    use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
    use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    enum Command {
        Present(Framebuffer, SyncSender<Result<()>>),
        Wait(Duration, SyncSender<Result<Vec<Event>>>),
        Close(SyncSender<Result<()>>),
        Reopen(SyncSender<Result<()>>),
        Shutdown,
    }

    #[derive(Debug)]
    pub(super) enum Event {
        CloseRequested,
        Destroyed,
        FocusGained,
        FocusLost,
        PointerMove {
            x: i32,
            y: i32,
        },
        PointerDown {
            x: i32,
            y: i32,
            button: &'static str,
        },
        PointerUp {
            x: i32,
            y: i32,
            button: &'static str,
        },
        PointerWheel {
            x: i32,
            y: i32,
            delta_x: i16,
            delta_y: i16,
            modifiers: ModifierSnapshot,
        },
        KeyDown {
            virtual_key: u32,
            repeated: bool,
        },
        KeyUp {
            virtual_key: u32,
        },
        TextInput {
            character: char,
        },
        TextComposition {
            phase: &'static str,
            text: String,
        },
        Resized {
            width: u32,
            height: u32,
        },
        DpiChanged {
            dpi: u32,
        },
    }

    #[derive(Debug)]
    pub(super) struct ModifierSnapshot {
        bits: u8,
    }

    impl ModifierSnapshot {
        const CTRL: u8 = 1;
        const SHIFT: u8 = 2;
        const ALT: u8 = 4;
        const META: u8 = 8;

        fn ctrl(&self) -> bool {
            self.bits & Self::CTRL != 0
        }
        fn shift(&self) -> bool {
            self.bits & Self::SHIFT != 0
        }
        fn alt(&self) -> bool {
            self.bits & Self::ALT != 0
        }
        fn meta(&self) -> bool {
            self.bits & Self::META != 0
        }
    }

    fn modifier_snapshot(value: ModifierState) -> ModifierSnapshot {
        let mut bits = 0;
        if value.ctrl() {
            bits |= ModifierSnapshot::CTRL;
        }
        if value.shift() {
            bits |= ModifierSnapshot::SHIFT;
        }
        if value.alt() {
            bits |= ModifierSnapshot::ALT;
        }
        if value.meta() {
            bits |= ModifierSnapshot::META;
        }
        ModifierSnapshot { bits }
    }

    fn button_name(value: MouseButton) -> &'static str {
        match value {
            MouseButton::Left => "left",
            MouseButton::Right => "right",
            MouseButton::Middle => "middle",
            MouseButton::X1 => "x1",
            MouseButton::X2 => "x2",
        }
    }

    fn phase_name(value: CompositionPhase) -> &'static str {
        match value {
            CompositionPhase::Started => "started",
            CompositionPhase::Updated => "updated",
            CompositionPhase::Committed => "committed",
            CompositionPhase::Canceled => "canceled",
        }
    }

    fn event(value: WindowEvent) -> Event {
        match value {
            WindowEvent::CloseRequested => Event::CloseRequested,
            WindowEvent::Destroyed => Event::Destroyed,
            WindowEvent::FocusGained => Event::FocusGained,
            WindowEvent::FocusLost => Event::FocusLost,
            WindowEvent::PointerMove { x, y } => Event::PointerMove { x, y },
            WindowEvent::PointerDown {
                x,
                y,
                button: value,
            } => Event::PointerDown {
                x,
                y,
                button: button_name(value),
            },
            WindowEvent::PointerUp {
                x,
                y,
                button: value,
            } => Event::PointerUp {
                x,
                y,
                button: button_name(value),
            },
            WindowEvent::PointerWheel {
                x,
                y,
                delta_x,
                delta_y,
                modifiers: value,
            } => Event::PointerWheel {
                x,
                y,
                delta_x,
                delta_y,
                modifiers: modifier_snapshot(value),
            },
            WindowEvent::KeyDown {
                virtual_key,
                repeated,
            } => Event::KeyDown {
                virtual_key,
                repeated,
            },
            WindowEvent::KeyUp { virtual_key } => Event::KeyUp { virtual_key },
            WindowEvent::TextInput { character } => Event::TextInput { character },
            WindowEvent::TextComposition { phase: value, text } => Event::TextComposition {
                phase: phase_name(value),
                text,
            },
            WindowEvent::Resized { width, height } => Event::Resized { width, height },
            WindowEvent::DpiChanged { dpi } => Event::DpiChanged { dpi },
        }
    }

    fn io_error(error: std::io::Error) -> MetisError {
        MetisError::from(error)
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "the host thread owns these values"
    )]
    fn host(config: WindowConfig, receiver: Receiver<Command>, ready: SyncSender<Result<()>>) {
        let mut surface = match NativeSurface::new(&config) {
            Ok(surface) => {
                let _ = ready.send(Ok(()));
                surface
            }
            Err(error) => {
                let _ = ready.send(Err(io_error(error)));
                return;
            }
        };
        while let Ok(command) = receiver.recv() {
            match command {
                Command::Present(framebuffer, reply) => {
                    let _ = reply.send(surface.present(&framebuffer).map_err(io_error));
                }
                Command::Wait(timeout, reply) => {
                    let result = surface
                        .wait_events(timeout)
                        .map(|events| events.into_iter().map(event).collect())
                        .map_err(io_error);
                    let _ = reply.send(result);
                }
                Command::Close(reply) => {
                    let _ = reply.send(surface.close().map_err(io_error));
                }
                Command::Reopen(reply) => {
                    let _ = reply.send(surface.reopen().map_err(io_error));
                }
                Command::Shutdown => {
                    let _ = surface.close();
                    return;
                }
            }
        }
        let _ = surface.close();
    }

    pub(super) struct Client {
        sender: Option<SyncSender<Command>>,
        join: Option<JoinHandle<()>>,
        pub(super) generation: u64,
        closed: bool,
    }

    impl Client {
        pub(super) fn new(title: &str, width: u32, height: u32, visibility: &str) -> Result<Self> {
            let visibility = match visibility {
                "hidden" => WindowVisibility::Hidden,
                "visible" => WindowVisibility::Visible,
                _ => {
                    return Err(MetisError::ui(
                        ErrorCode::MalformedPayload,
                        "visibility must be 'visible' or 'hidden'",
                    ));
                }
            };
            let config = WindowConfig::with_visibility(title, width, height, visibility)
                .map_err(io_error)?;
            let (sender, receiver) = sync_channel(8);
            let (ready_sender, ready_receiver) = sync_channel(1);
            let join = thread::Builder::new()
                .name("metis-python-native-host".to_owned())
                .spawn(move || host(config, receiver, ready_sender))
                .map_err(io_error)?;
            match ready_receiver.recv() {
                Ok(Ok(())) => Ok(Self {
                    sender: Some(sender),
                    join: Some(join),
                    generation: 0,
                    closed: false,
                }),
                Ok(Err(error)) => {
                    let _ = join.join();
                    Err(error)
                }
                Err(_) => {
                    let _ = join.join();
                    Err(MetisError::transport(
                        ErrorCode::ConnectionClosed,
                        "native host stopped before readiness",
                    ))
                }
            }
        }

        fn check_generation(&self, generation: u64) -> Result<()> {
            if self.generation != generation {
                return Err(MetisError::ui(
                    ErrorCode::RenderFailure,
                    "NativeApplication generation is stale",
                ));
            }
            if self.closed {
                return Err(MetisError::ui(
                    ErrorCode::RenderFailure,
                    "NativeApplication is closed",
                ));
            }
            Ok(())
        }

        #[expect(
            clippy::needless_pass_by_value,
            reason = "the response receiver is consumed by this request"
        )]
        fn request<T>(&self, command: Command, receiver: Receiver<Result<T>>) -> Result<T> {
            self.sender
                .as_ref()
                .ok_or_else(|| {
                    MetisError::transport(ErrorCode::ConnectionClosed, "native host is stopped")
                })?
                .send(command)
                .map_err(|_| {
                    MetisError::transport(ErrorCode::ConnectionClosed, "native host stopped")
                })?;
            receiver.recv().map_err(|_| {
                MetisError::transport(ErrorCode::ConnectionClosed, "native host stopped")
            })?
        }

        pub(super) fn present(&self, generation: u64, framebuffer: Framebuffer) -> Result<()> {
            self.check_generation(generation)?;
            let (sender, receiver) = sync_channel(1);
            self.request(Command::Present(framebuffer, sender), receiver)
        }

        pub(super) fn wait(&self, generation: u64, timeout: Duration) -> Result<Vec<Event>> {
            self.check_generation(generation)?;
            let (sender, receiver) = sync_channel(1);
            self.request(Command::Wait(timeout, sender), receiver)
        }

        pub(super) fn close(&mut self, generation: u64) -> Result<()> {
            self.check_generation(generation)?;
            let (sender, receiver) = sync_channel(1);
            self.request(Command::Close(sender), receiver)?;
            self.closed = true;
            Ok(())
        }

        pub(super) fn reopen(&mut self) -> Result<u64> {
            if !self.closed {
                return Err(MetisError::ui(
                    ErrorCode::RenderFailure,
                    "NativeApplication must be closed before reopen",
                ));
            }
            let (sender, receiver) = sync_channel(1);
            self.request(Command::Reopen(sender), receiver)?;
            self.generation = self
                .generation
                .checked_add(1)
                .ok_or_else(|| MetisError::ui(ErrorCode::RenderFailure, "generation exhausted"))?;
            self.closed = false;
            Ok(self.generation)
        }
    }

    impl Drop for Client {
        fn drop(&mut self) {
            if let Some(sender) = self.sender.take() {
                let _ = sender.try_send(Command::Shutdown);
                drop(sender);
            }
            if let Some(join) = self.join.take() {
                let _ = join.join();
            }
        }
    }

    pub(super) fn frame_from_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<Framebuffer> {
        let pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|value| value.checked_mul(4))
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                MetisError::ui(ErrorCode::SurfaceAllocationError, "frame size overflow")
            })?;
        if rgba.len() != pixels {
            return Err(MetisError::ui(
                ErrorCode::RenderFailure,
                "RGBA frame length does not match the native window",
            ));
        }
        let mut framebuffer = Framebuffer::new(width, height)?;
        for (index, channels) in rgba.chunks_exact(4).enumerate() {
            let index = u64::try_from(index).map_err(|_| {
                MetisError::ui(ErrorCode::SurfaceAllocationError, "frame index overflow")
            })?;
            let x = u32::try_from(index % u64::from(width)).map_err(|_| {
                MetisError::ui(
                    ErrorCode::SurfaceAllocationError,
                    "frame coordinate overflow",
                )
            })?;
            let y = u32::try_from(index / u64::from(width)).map_err(|_| {
                MetisError::ui(
                    ErrorCode::SurfaceAllocationError,
                    "frame coordinate overflow",
                )
            })?;
            let color =
                metis_platform::Color::rgba(channels[0], channels[1], channels[2], channels[3]);
            let x = i32::try_from(x).map_err(|_| {
                MetisError::ui(
                    ErrorCode::SurfaceAllocationError,
                    "frame coordinate exceeds i32",
                )
            })?;
            let y = i32::try_from(y).map_err(|_| {
                MetisError::ui(
                    ErrorCode::SurfaceAllocationError,
                    "frame coordinate exceeds i32",
                )
            })?;
            framebuffer.set_pixel(x, y, color);
        }
        Ok(framebuffer)
    }

    pub(super) fn append_event<'py>(
        py: Python<'py>,
        list: &Bound<'py, PyList>,
        value: Event,
    ) -> PyResult<()> {
        let item = PyDict::new(py);
        macro_rules! set {
            ($key:literal, $value:expr) => {{ item.set_item($key, $value)? }};
        }
        match value {
            Event::CloseRequested => set!("kind", "close_requested"),
            Event::Destroyed => set!("kind", "destroyed"),
            Event::FocusGained => set!("kind", "focus_gained"),
            Event::FocusLost => set!("kind", "focus_lost"),
            Event::PointerMove { x, y } => {
                set!("kind", "pointer_move");
                set!("x", x);
                set!("y", y);
            }
            Event::PointerDown { x, y, button } => {
                set!("kind", "pointer_down");
                set!("x", x);
                set!("y", y);
                set!("button", button);
            }
            Event::PointerUp { x, y, button } => {
                set!("kind", "pointer_up");
                set!("x", x);
                set!("y", y);
                set!("button", button);
            }
            Event::PointerWheel {
                x,
                y,
                delta_x,
                delta_y,
                modifiers,
            } => {
                set!("kind", "pointer_wheel");
                set!("x", x);
                set!("y", y);
                set!("delta_x", delta_x);
                set!("delta_y", delta_y);
                set!("ctrl", modifiers.ctrl());
                set!("shift", modifiers.shift());
                set!("alt", modifiers.alt());
                set!("meta", modifiers.meta());
            }
            Event::KeyDown {
                virtual_key,
                repeated,
            } => {
                set!("kind", "key_down");
                set!("virtual_key", virtual_key);
                set!("repeated", repeated);
            }
            Event::KeyUp { virtual_key } => {
                set!("kind", "key_up");
                set!("virtual_key", virtual_key);
            }
            Event::TextInput { character } => {
                set!("kind", "text_input");
                set!("character", character);
            }
            Event::TextComposition { phase, text } => {
                set!("kind", "text_composition");
                set!("phase", phase);
                set!("text", text);
            }
            Event::Resized { width, height } => {
                set!("kind", "resized");
                set!("width", width);
                set!("height", height);
            }
            Event::DpiChanged { dpi } => {
                set!("kind", "dpi_changed");
                set!("dpi", dpi);
            }
        }
        list.append(item)
    }
}

#[cfg(windows)]
use windows_host::{append_event, frame_from_rgba};

/// Rust-owned native window and event host for Python applications.
#[cfg(windows)]
#[pyclass(name = "NativeApplication")]
pub(crate) struct NativeApplication {
    client: std::sync::Mutex<windows_host::Client>,
    width: u32,
    height: u32,
}

/// Unsupported-platform marker retaining a stable Python API on non-Windows hosts.
#[cfg(not(windows))]
#[pyclass(name = "NativeApplication")]
pub(crate) struct NativeApplication;

#[cfg(windows)]
#[pymethods]
impl NativeApplication {
    /// Creates a visible or hidden bounded native window.
    #[new]
    fn new(title: &str, width: u32, height: u32, visibility: &str) -> PyResult<Self> {
        let client = windows_host::Client::new(title, width, height, visibility)
            .map_err(|error| map_error(&error))?;
        Ok(Self {
            client: std::sync::Mutex::new(client),
            width,
            height,
        })
    }

    /// Returns the current close/reopen generation.
    #[getter]
    fn generation(&self) -> PyResult<u64> {
        self.client
            .lock()
            .map(|client| client.generation)
            .map_err(|_| {
                map_error(&MetisError::ui(
                    ErrorCode::RenderFailure,
                    "native host lock is poisoned",
                ))
            })
    }

    /// Presents one row-major RGBA frame.
    fn present(&self, generation: u64, rgba: &[u8]) -> PyResult<()> {
        let framebuffer =
            frame_from_rgba(self.width, self.height, rgba).map_err(|error| map_error(&error))?;
        let client = self.client.lock().map_err(|_| {
            map_error(&MetisError::ui(
                ErrorCode::RenderFailure,
                "native host lock is poisoned",
            ))
        })?;
        client
            .present(generation, framebuffer)
            .map_err(|error| map_error(&error))
    }

    /// Waits for one bounded native event batch.
    fn wait_events<'py>(
        &self,
        py: Python<'py>,
        generation: u64,
        timeout_ms: u32,
    ) -> PyResult<Bound<'py, PyList>> {
        if timeout_ms > windows_host::MAX_WAIT_MILLISECONDS {
            return Err(map_error(&MetisError::transport(
                ErrorCode::Timeout,
                "native wait exceeds the provider limit",
            )));
        }
        let client = self.client.lock().map_err(|_| {
            map_error(&MetisError::ui(
                ErrorCode::RenderFailure,
                "native host lock is poisoned",
            ))
        })?;
        let events = client
            .wait(
                generation,
                std::time::Duration::from_millis(u64::from(timeout_ms)),
            )
            .map_err(|error| map_error(&error))?;
        let list = PyList::empty(py);
        for event in events {
            append_event(py, &list, event)?;
        }
        Ok(list)
    }

    /// Closes the native window and invalidates its generation.
    fn close(&self, generation: u64) -> PyResult<()> {
        let mut client = self.client.lock().map_err(|_| {
            map_error(&MetisError::ui(
                ErrorCode::RenderFailure,
                "native host lock is poisoned",
            ))
        })?;
        client.close(generation).map_err(|error| map_error(&error))
    }

    /// Reopens a closed window and returns its new generation.
    fn reopen(&self) -> PyResult<u64> {
        let mut client = self.client.lock().map_err(|_| {
            map_error(&MetisError::ui(
                ErrorCode::RenderFailure,
                "native host lock is poisoned",
            ))
        })?;
        client.reopen().map_err(|error| map_error(&error))
    }
}

#[cfg(not(windows))]
#[pymethods]
impl NativeApplication {
    /// Reports that no native provider is installed for this target.
    #[new]
    fn new(_title: &str, _width: u32, _height: u32, _visibility: &str) -> PyResult<Self> {
        Err(map_error(&unsupported_error()))
    }
}

#[cfg(windows)]
pub(crate) fn assert_thread_safe() {
    fn assert_type<T: Send + Sync>() {}
    assert_type::<NativeApplication>();
}

#[cfg(not(windows))]
pub(crate) fn assert_thread_safe() {
    fn assert_type<T: Send + Sync>() {}
    assert_type::<NativeApplication>();
}
