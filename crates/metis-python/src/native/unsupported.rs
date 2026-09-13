use crate::error::map_error;
use metis_core::error::{ErrorCode, MetisError};
use pyo3::prelude::*;

fn unsupported_error() -> MetisError {
    MetisError::ui(
        ErrorCode::UnsupportedPlatformEvent,
        "NativeApplication is unavailable on this platform",
    )
}

/// Unsupported-platform marker retaining a stable Python API on non-Windows hosts.
#[pyclass(name = "NativeApplication")]
pub(crate) struct NativeApplication;

#[pymethods]
impl NativeApplication {
    /// Reports that no native provider is installed for this target.
    #[new]
    fn new(_title: &str, _width: u32, _height: u32, _visibility: &str) -> PyResult<Self> {
        Err(map_error(&unsupported_error()))
    }
}

pub(crate) fn assert_thread_safe() {
    fn assert_type<T: Send + Sync>() {}
    assert_type::<NativeApplication>();
}
