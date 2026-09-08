//! Thin `PyO3` bindings for the Metis Rust application framework.
//!
//! The binding converts Python values into validated Rust types, delegates
//! computation to `metis-backend`, and converts the result back to Python.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod clinical;

use clinical::{
    DrugConcentration, InfusionResult, PatientWeight, SafetyEnvelope, TargetDose,
    calculate_infusion_rate,
};
use pyo3::prelude::*;

/// Native extension module loaded as `metis._metis`.
#[pymodule]
fn _metis(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PatientWeight>()?;
    module.add_class::<DrugConcentration>()?;
    module.add_class::<TargetDose>()?;
    module.add_class::<SafetyEnvelope>()?;
    module.add_class::<InfusionResult>()?;
    module.add_function(wrap_pyfunction!(calculate_infusion_rate, module)?)?;
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
