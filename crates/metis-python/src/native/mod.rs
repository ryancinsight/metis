//! Thin Python access to the Rust-owned native application host.

use crate::error::map_error;
use metis_core::error::{ErrorCode, MetisError};
use pyo3::prelude::*;
use pyo3::types::PyList;

#[cfg(windows)]
mod events;
#[cfg(windows)]
mod host;

#[cfg(not(windows))]
fn unsupported_error() -> MetisError {
    MetisError::ui(
        ErrorCode::UnsupportedPlatformEvent,
        "NativeApplication is unavailable on this platform",
    )
}

#[cfg(windows)]
use events::append_event;
#[cfg(windows)]
use host::{Client, frame_from_rgba};

/// Rust-owned native window and event host for Python applications.
#[cfg(windows)]
#[pyclass(name = "NativeApplication")]
pub(crate) struct NativeApplication {
    client: std::sync::Mutex<Client>,
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
        let client =
            Client::new(title, width, height, visibility).map_err(|error| map_error(&error))?;
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
        if timeout_ms > host::MAX_WAIT_MILLISECONDS {
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
