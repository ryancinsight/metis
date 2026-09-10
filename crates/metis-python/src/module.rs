use super::application::Application;
use super::clinical::{
    DrugConcentration, InfusionResult, PatientWeight, SafetyEnvelope, TargetDose,
    calculate_infusion_rate,
};
use super::presentation::{Canvas, PyRasterImage, PyRect};
use pyo3::prelude::*;

fn assert_thread_safe<T: Send + Sync>() {}

fn audit_exposed_types() {
    assert_thread_safe::<Application>();
    assert_thread_safe::<PatientWeight>();
    assert_thread_safe::<DrugConcentration>();
    assert_thread_safe::<TargetDose>();
    assert_thread_safe::<SafetyEnvelope>();
    assert_thread_safe::<InfusionResult>();
    assert_thread_safe::<PyRect>();
    assert_thread_safe::<PyRasterImage>();
    assert_thread_safe::<Canvas>();
}

/// Native extension module loaded as `metis._metis`.
#[pymodule(gil_used = false)]
fn _metis(module: &Bound<'_, PyModule>) -> PyResult<()> {
    audit_exposed_types();
    module.add_class::<Application>()?;
    module.add_class::<PatientWeight>()?;
    module.add_class::<DrugConcentration>()?;
    module.add_class::<TargetDose>()?;
    module.add_class::<SafetyEnvelope>()?;
    module.add_class::<InfusionResult>()?;
    module.add_class::<PyRect>()?;
    module.add_class::<PyRasterImage>()?;
    module.add_class::<Canvas>()?;
    module.add_function(wrap_pyfunction!(calculate_infusion_rate, module)?)?;
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
