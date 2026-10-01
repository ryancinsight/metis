use super::events::append_event;
use super::host::{Client, MAX_WAIT_MILLISECONDS, frame_from_rgba};
use crate::error::map_error;
use metis_core::error::{ErrorCode, MetisError};
use pyo3::prelude::*;
use pyo3::types::PyList;
use std::sync::{
    Mutex, MutexGuard,
    atomic::{AtomicU64, Ordering},
};

/// Rust-owned native window and event host for Python applications.
#[pyclass(name = "NativeApplication")]
pub(crate) struct NativeApplication {
    client: Mutex<Client>,
    /// Mirror of the client's generation, written only by `reopen` while it
    /// holds the client lock. Reading it needs no lock, which a pending
    /// `wait_events` holds for its whole timeout.
    generation: AtomicU64,
    width: u32,
    height: u32,
}

impl NativeApplication {
    fn lock_client(&self) -> Result<MutexGuard<'_, Client>, MetisError> {
        self.client
            .lock()
            .map_err(|_| MetisError::ui(ErrorCode::RenderFailure, "native host lock is poisoned"))
    }
}

#[pymethods]
impl NativeApplication {
    /// Creates a visible or hidden bounded native window.
    #[new]
    fn new(
        py: Python<'_>,
        title: &str,
        width: u32,
        height: u32,
        visibility: &str,
    ) -> PyResult<Self> {
        // Window creation can take the whole readiness deadline, so the
        // interpreter stays detached for it.
        let client = py
            .detach(|| Client::new(title, width, height, visibility))
            .map_err(|error| map_error(&error))?;
        Ok(Self {
            generation: AtomicU64::new(client.generation),
            client: Mutex::new(client),
            width,
            height,
        })
    }

    /// Returns the current close/reopen generation.
    #[getter]
    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Presents one row-major RGBA frame.
    fn present(&self, py: Python<'_>, generation: u64, rgba: &[u8]) -> PyResult<()> {
        let framebuffer =
            frame_from_rgba(self.width, self.height, rgba).map_err(|error| map_error(&error))?;
        let result = py.detach(|| self.lock_client()?.present(generation, framebuffer));
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
            self.lock_client()?.wait(
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
        let result = py.detach(|| self.lock_client()?.close(generation));
        result.map_err(|error| map_error(&error))
    }

    /// Reopens a closed window and returns its new generation.
    fn reopen(&self, py: Python<'_>) -> PyResult<u64> {
        let result = py.detach(|| {
            let mut client = self.lock_client()?;
            let generation = client.reopen()?;
            self.generation.store(generation, Ordering::Release);
            Ok::<_, MetisError>(generation)
        });
        result.map_err(|error| map_error(&error))
    }
}

pub(crate) fn assert_thread_safe() {
    fn assert_type<T: Send + Sync>() {}
    assert_type::<NativeApplication>();
}
