//! Python value objects for the validated clinical backend contract.

use crate::error::map_error;
use metis_backend::clinical::{
    DrugConcentrationMgMl, PatientWeightKg, SafetyEnvelope as RustSafetyEnvelope, TargetDoseRate,
    calculate_infusion_rate as calculate,
};
use pyo3::prelude::*;

/// Validated patient weight in kilograms.
#[pyclass(frozen, name = "PatientWeight")]
pub(crate) struct PatientWeight {
    inner: PatientWeightKg,
}

#[pymethods]
impl PatientWeight {
    /// Creates a bounded patient weight.
    #[new]
    fn new(kilograms: f64) -> PyResult<Self> {
        PatientWeightKg::new(kilograms)
            .map(|inner| Self { inner })
            .map_err(|error| map_error(&error))
    }

    /// Returns the validated value in kilograms.
    #[getter]
    fn kilograms(&self) -> f64 {
        self.inner.get()
    }
}

/// Validated drug concentration in milligrams per milliliter.
#[pyclass(frozen, name = "DrugConcentration")]
pub(crate) struct DrugConcentration {
    inner: DrugConcentrationMgMl,
}

#[pymethods]
impl DrugConcentration {
    /// Creates a bounded drug concentration.
    #[new]
    fn new(milligrams_per_milliliter: f64) -> PyResult<Self> {
        DrugConcentrationMgMl::new(milligrams_per_milliliter)
            .map(|inner| Self { inner })
            .map_err(|error| map_error(&error))
    }

    /// Returns the validated value in milligrams per milliliter.
    #[getter]
    fn milligrams_per_milliliter(&self) -> f64 {
        self.inner.get()
    }
}

/// Validated target dosage in micrograms per kilogram per minute.
#[pyclass(frozen, name = "TargetDose")]
pub(crate) struct TargetDose {
    inner: TargetDoseRate,
}

#[pymethods]
impl TargetDose {
    /// Creates a bounded target dose.
    #[new]
    fn new(micrograms_per_kilogram_per_minute: f64) -> PyResult<Self> {
        TargetDoseRate::new(micrograms_per_kilogram_per_minute)
            .map(|inner| Self { inner })
            .map_err(|error| map_error(&error))
    }

    /// Returns the validated value in micrograms per kilogram per minute.
    #[getter]
    fn micrograms_per_kilogram_per_minute(&self) -> f64 {
        self.inner.get()
    }
}

/// Validated clinical rate ceilings.
#[pyclass(frozen, name = "SafetyEnvelope")]
pub(crate) struct SafetyEnvelope {
    inner: RustSafetyEnvelope,
}

#[pymethods]
impl SafetyEnvelope {
    /// Creates rate ceilings, using the Rust demonstration defaults when omitted.
    #[new]
    #[pyo3(signature = (
        adult_rate_ml_hr = 300.0,
        pediatric_rate_ml_hr = 50.0,
        pediatric_weight_threshold_kg = 35.0
    ))]
    fn new(
        adult_rate_ml_hr: f64,
        pediatric_rate_ml_hr: f64,
        pediatric_weight_threshold_kg: f64,
    ) -> PyResult<Self> {
        RustSafetyEnvelope::new(
            adult_rate_ml_hr,
            pediatric_rate_ml_hr,
            pediatric_weight_threshold_kg,
        )
        .map(|inner| Self { inner })
        .map_err(|error| map_error(&error))
    }
}

/// Input-sensitive infusion result returned by the Rust backend.
#[pyclass(frozen, name = "InfusionResult")]
pub(crate) struct InfusionResult {
    rate_ml_hr: f64,
    drug_rate_mg_hr: f64,
    is_pediatric: bool,
}

#[pymethods]
impl InfusionResult {
    /// Returns the calculated infusion rate in milliliters per hour.
    #[getter]
    fn rate_ml_hr(&self) -> f64 {
        self.rate_ml_hr
    }

    /// Returns the calculated drug delivery rate in milligrams per hour.
    #[getter]
    fn drug_rate_mg_hr(&self) -> f64 {
        self.drug_rate_mg_hr
    }

    /// Returns whether the pediatric ceiling was selected.
    #[getter]
    fn is_pediatric(&self) -> bool {
        self.is_pediatric
    }
}

impl From<metis_backend::clinical::ClinicalCalculationResult> for InfusionResult {
    fn from(result: metis_backend::clinical::ClinicalCalculationResult) -> Self {
        Self {
            rate_ml_hr: result.rate_ml_hr,
            drug_rate_mg_hr: result.drug_rate_mg_hr,
            is_pediatric: result.is_pediatric,
        }
    }
}

/// Calculates an infusion rate using the validated Rust clinical contract.
#[pyfunction]
#[pyo3(signature = (weight, concentration, dose, envelope = None))]
pub(crate) fn calculate_infusion_rate(
    py: Python<'_>,
    weight: &PatientWeight,
    concentration: &DrugConcentration,
    dose: &TargetDose,
    envelope: Option<&SafetyEnvelope>,
) -> PyResult<InfusionResult> {
    let weight = weight.inner;
    let concentration = concentration.inner;
    let dose = dose.inner;
    let envelope = envelope.map_or_else(RustSafetyEnvelope::default, |value| value.inner);

    py.detach(move || {
        calculate(weight, concentration, dose, &envelope)
            .map(InfusionResult::from)
            .map_err(|error| map_error(&error))
    })
}
