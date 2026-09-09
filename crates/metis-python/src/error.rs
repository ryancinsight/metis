//! Conversion of the Rust error taxonomy into Python exceptions.

use metis_core::MetisError;
use pyo3::PyErr;
use pyo3::exceptions::PyValueError;

pub(crate) fn map_error(error: &MetisError) -> PyErr {
    PyValueError::new_err(format!(
        "{} [{} | {}]",
        error.message,
        error.code.as_str(),
        error.trace_id
    ))
}
