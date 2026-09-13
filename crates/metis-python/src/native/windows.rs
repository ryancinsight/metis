use super::events::append_event;
use super::host::{Client, MAX_WAIT_MILLISECONDS, frame_from_rgba};
use crate::error::map_error;
use metis_core::error::{ErrorCode, MetisError};
use pyo3::prelude::*;
use pyo3::types::PyList;

/// Rust-owned native window and event host for Python applications.
#[pyclass(name = "NativeApplication")]
pub(crate) struct NativeApplication {
    client: std::sync::Mutex<Client>,
    width: u32,
    height: u32,
}

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
    fn present(&self, py: Python<'_>, generation: u64, rgba: &[u8]) -> PyResult<()> {
        let framebuffer =
            frame_from_rgba(self.width, self.height, rgba).map_err(|error| map_error(&error))?;
        let result = py.detach(|| {
            let client = self.client.lock().map_err(|_| {
                MetisError::ui(ErrorCode::RenderFailure, "native host lock is poisoned")
            })?;
            client.present(generation, framebuffer)
        });
        result.map_err(|error| map_error(&error))
    }

    /// Waits for one bounded native event batch.
    fn wait_events<'py>(
        &self,
        py: Python<'py>,
        generation: u64,
        timeout_ms: u32,
    ) -> PyResult<Bound<'py, PyList>> {
        if timeout_ms > MAX_WAIT_MILLISECONDS {
            return Err(map_error(&MetisError::transport(
                ErrorCode::Timeout,
                "native wait exceeds the provider limit",
            )));
        }
        let events = py.detach(|| {
            let client = self.client.lock().map_err(|_| {
                MetisError::ui(ErrorCode::RenderFailure, "native host lock is poisoned")
            })?;
            client.wait(
                generation,
                std::time::Duration::from_millis(u64::from(timeout_ms)),
            )
        });
        let events = events.map_err(|error| map_error(&error))?;
        let list = PyList::empty(py);
        for event in events {
            append_event(py, &list, event)?;
        }
        Ok(list)
    }

    /// Closes the native window and invalidates its generation.
    fn close(&self, py: Python<'_>, generation: u64) -> PyResult<()> {
        let result = py.detach(|| {
            let mut client = self.client.lock().map_err(|_| {
                MetisError::ui(ErrorCode::RenderFailure, "native host lock is poisoned")
            })?;
            client.close(generation)
        });
        result.map_err(|error| map_error(&error))
    }

    /// Reopens a closed window and returns its new generation.
    fn reopen(&self, py: Python<'_>) -> PyResult<u64> {
        let result = py.detach(|| {
            let mut client = self.client.lock().map_err(|_| {
                MetisError::ui(ErrorCode::RenderFailure, "native host lock is poisoned")
            })?;
            client.reopen()
        });
        result.map_err(|error| map_error(&error))
    }
}

pub(crate) fn assert_thread_safe() {
    fn assert_type<T: Send + Sync>() {}
    assert_type::<NativeApplication>();
}
