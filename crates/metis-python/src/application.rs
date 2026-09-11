//! Rust-owned, bounded application lifecycle exposed through `PyO3`.

use crate::error::map_error;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_platform::{Color, PlatformEvent, PlatformSurface};
use pyo3::prelude::*;
use pyo3::sync::MutexExt;
use pyo3::types::{PyBytes, PyDict};
use std::sync::{Mutex, MutexGuard};

fn lifecycle_error(message: &'static str) -> MetisError {
    MetisError::ui(ErrorCode::RenderFailure, message)
}

struct ApplicationState {
    generation: u64,
    surface: Option<PlatformSurface>,
}

/// Bounded Rust-owned software application state for Python hosts.
///
/// The object owns one virtual [`PlatformSurface`]. Python supplies input and
/// reads completed frames; it does not provide callbacks or an event loop.
#[pyclass(name = "Application")]
pub(crate) struct Application {
    state: Mutex<ApplicationState>,
}

impl Application {
    fn lock(&self, py: Python<'_>) -> PyResult<MutexGuard<'_, ApplicationState>> {
        self.state
            .lock_py_attached(py)
            .map_err(|_| map_error(&lifecycle_error("Application state lock is poisoned")))
    }

    fn active_surface(
        state: &mut ApplicationState,
        generation: u64,
    ) -> Result<&mut PlatformSurface> {
        if state.generation != generation {
            return Err(lifecycle_error("Application generation is stale"));
        }
        state
            .surface
            .as_mut()
            .ok_or_else(|| lifecycle_error("Application is closed"))
    }

    fn active_surface_ref(state: &ApplicationState, generation: u64) -> Result<&PlatformSurface> {
        if state.generation != generation {
            return Err(lifecycle_error("Application generation is stale"));
        }
        state
            .surface
            .as_ref()
            .ok_or_else(|| lifecycle_error("Application is closed"))
    }
}

#[pymethods]
impl Application {
    /// Allocates a bounded virtual application surface.
    #[new]
    fn new(width: u32, height: u32) -> PyResult<Self> {
        let surface = PlatformSurface::new(width, height).map_err(|error| map_error(&error))?;
        Ok(Self {
            state: Mutex::new(ApplicationState {
                generation: 0,
                surface: Some(surface),
            }),
        })
    }

    /// Returns the generation token required by operations on the current surface.
    #[getter]
    fn generation(&self, py: Python<'_>) -> PyResult<u64> {
        Ok(self.lock(py)?.generation)
    }

    /// Returns the current surface width.
    #[getter]
    fn width(&self, py: Python<'_>) -> PyResult<u32> {
        let result = {
            let state = self.lock(py)?;
            state
                .surface
                .as_ref()
                .map(|surface| surface.framebuffer.width())
                .ok_or_else(|| lifecycle_error("Application is closed"))
        };
        result.map_err(|error| map_error(&error))
    }

    /// Returns the current surface height.
    #[getter]
    fn height(&self, py: Python<'_>) -> PyResult<u32> {
        let result = {
            let state = self.lock(py)?;
            state
                .surface
                .as_ref()
                .map(|surface| surface.framebuffer.height())
                .ok_or_else(|| lifecycle_error("Application is closed"))
        };
        result.map_err(|error| map_error(&error))
    }

    /// Clears the current framebuffer after validating its generation token.
    fn clear(
        &self,
        py: Python<'_>,
        generation: u64,
        red: u8,
        green: u8,
        blue: u8,
        alpha: u8,
    ) -> PyResult<()> {
        let result = {
            let mut state = self.lock(py)?;
            Self::active_surface(&mut state, generation).map(|surface| {
                surface
                    .framebuffer
                    .clear(Color::rgba(red, green, blue, alpha));
            })
        };
        result.map_err(|error| map_error(&error))
    }

    /// Returns a bounded copy of the current row-major RGBA framebuffer.
    fn to_rgba<'py>(&self, py: Python<'py>, generation: u64) -> PyResult<Bound<'py, PyBytes>> {
        let bytes: Result<Vec<u8>> = py.detach(|| {
            let state = self
                .state
                .lock()
                .map_err(|_| lifecycle_error("Application state lock is poisoned"))?;
            let surface = Self::active_surface_ref(&state, generation)?;
            let mut bytes = Vec::new();
            let pixels = u64::from(surface.framebuffer.width())
                .checked_mul(u64::from(surface.framebuffer.height()))
                .and_then(|count| count.checked_mul(4))
                .and_then(|count| usize::try_from(count).ok())
                .ok_or_else(|| lifecycle_error("Framebuffer byte count overflows"))?;
            bytes
                .try_reserve_exact(pixels)
                .map_err(|_| lifecycle_error("Framebuffer byte allocation failed"))?;
            for y in 0..surface.framebuffer.height() {
                for x in 0..surface.framebuffer.width() {
                    let x = i32::try_from(x).expect("invariant: framebuffer width fits i32");
                    let y = i32::try_from(y).expect("invariant: framebuffer height fits i32");
                    let color = surface.framebuffer.get_pixel(x, y);
                    bytes.extend([color.r, color.g, color.b, color.a]);
                }
            }
            Ok(bytes)
        });
        let bytes = bytes.map_err(|error| map_error(&error))?;
        Ok(PyBytes::new(py, &bytes))
    }

    /// Enqueues a key event in the bounded FIFO queue.
    fn key_down(&self, py: Python<'_>, generation: u64, key: u32) -> PyResult<()> {
        self.push_event(py, generation, PlatformEvent::KeyDown(key))
    }

    /// Enqueues a pointer press in the bounded FIFO queue.
    fn pointer_down(&self, py: Python<'_>, generation: u64, x: i32, y: i32) -> PyResult<()> {
        self.push_event(py, generation, PlatformEvent::PointerDown { x, y })
    }

    /// Enqueues a Unicode character event in the bounded FIFO queue.
    fn char_input(&self, py: Python<'_>, generation: u64, value: char) -> PyResult<()> {
        self.push_event(py, generation, PlatformEvent::CharInput(value))
    }

    /// Enqueues a quit event in the bounded FIFO queue.
    fn quit(&self, py: Python<'_>, generation: u64) -> PyResult<()> {
        self.push_event(py, generation, PlatformEvent::Quit)
    }

    /// Removes the next event as a dictionary, preserving FIFO order.
    fn poll_event<'py>(
        &self,
        py: Python<'py>,
        generation: u64,
    ) -> PyResult<Option<Bound<'py, PyDict>>> {
        let event = {
            let mut state = self.lock(py)?;
            Self::active_surface(&mut state, generation).map(PlatformSurface::poll_event)
        };
        let event = event.map_err(|error| map_error(&error))?;
        let Some(event) = event else {
            return Ok(None);
        };
        let result = PyDict::new(py);
        match event {
            PlatformEvent::KeyDown(key) => {
                result.set_item("kind", "key_down")?;
                result.set_item("key", key)?;
            }
            PlatformEvent::PointerDown { x, y } => {
                result.set_item("kind", "pointer_down")?;
                result.set_item("x", x)?;
                result.set_item("y", y)?;
            }
            PlatformEvent::CharInput(value) => {
                result.set_item("kind", "char_input")?;
                result.set_item("value", value)?;
            }
            PlatformEvent::Quit => {
                result.set_item("kind", "quit")?;
            }
            other => {
                result.set_item("kind", format!("{other:?}"))?;
            }
        }
        Ok(Some(result))
    }

    /// Closes the surface and invalidates the supplied generation token.
    fn close(&self, py: Python<'_>, generation: u64) -> PyResult<()> {
        let result = {
            let mut state = self.lock(py)?;
            if state.generation != generation {
                Err(lifecycle_error("Application generation is stale"))
            } else if state.surface.is_none() {
                Err(lifecycle_error("Application is closed"))
            } else {
                state.surface = None;
                Ok(())
            }
        };
        result.map_err(|error| map_error(&error))
    }

    /// Reopens a closed surface and returns its new generation token.
    fn reopen(&self, py: Python<'_>, width: u32, height: u32) -> PyResult<u64> {
        let result: Result<u64> = {
            let mut state = self.lock(py)?;
            (|| {
                if state.surface.is_some() {
                    return Err(lifecycle_error("Application must be closed before reopen"));
                }
                let surface = PlatformSurface::new(width, height)?;
                let generation = state
                    .generation
                    .checked_add(1)
                    .ok_or_else(|| lifecycle_error("Application generation exhausted"))?;
                state.generation = generation;
                state.surface = Some(surface);
                Ok(generation)
            })()
        };
        result.map_err(|error| map_error(&error))
    }
}

impl Application {
    fn push_event(&self, py: Python<'_>, generation: u64, event: PlatformEvent) -> PyResult<()> {
        let result = {
            let mut state = self.lock(py)?;
            Self::active_surface(&mut state, generation)
                .and_then(|surface| surface.push_event(event))
        };
        result.map_err(|error| map_error(&error))
    }
}
