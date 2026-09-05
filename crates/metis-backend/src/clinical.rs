//! Demonstration infusion arithmetic with explicit numerical and configured rate bounds.
//!
//! Bounds are example engineering inputs, not validated clinical guidance or
//! evidence of regulatory compliance. Arithmetic uses the wire contract's binary64.

use metis_core::error::{ErrorCode, MetisError, Result};

/// Bounded patient weight in kilograms (valid range: 0.25 kg to 500.0 kg).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PatientWeightKg(f64);

impl PatientWeightKg {
    /// Minimum weight admitted by this demonstration.
    pub const MIN_WEIGHT_KG: f64 = 0.25;
    /// Maximum weight admitted by this demonstration.
    pub const MAX_WEIGHT_KG: f64 = 500.0;

    /// Creates a validated `PatientWeightKg` or returns `MetisError`.
    ///
    /// # Errors
    /// Rejects nonfinite values and values outside the documented interval.
    pub fn new(weight: f64) -> Result<Self> {
        if weight.is_nan() || weight.is_infinite() {
            return Err(MetisError::clinical(
                ErrorCode::NumericInstability,
                "Patient weight cannot be NaN or Infinite",
            ));
        }
        if !(Self::MIN_WEIGHT_KG..=Self::MAX_WEIGHT_KG).contains(&weight) {
            return Err(MetisError::clinical(
                ErrorCode::InvalidPatientWeight,
                format!(
                    "Weight {} kg outside demonstration bounds [{}, {}]",
                    weight,
                    Self::MIN_WEIGHT_KG,
                    Self::MAX_WEIGHT_KG
                ),
            ));
        }
        Ok(Self(weight))
    }

    /// Returns the numerical weight value.
    #[must_use]
    pub const fn get(&self) -> f64 {
        self.0
    }
}

/// Drug concentration in milligrams per milliliter (valid range: 0.001 to 1000.0 mg/mL).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrugConcentrationMgMl(f64);

impl DrugConcentrationMgMl {
    /// Minimum concentration admitted by this demonstration.
    pub const MIN_CONC: f64 = 0.001;
    /// Maximum concentration admitted by this demonstration.
    pub const MAX_CONC: f64 = 1000.0;

    /// Creates a validated `DrugConcentrationMgMl` or returns `MetisError`.
    ///
    /// # Errors
    /// Rejects nonfinite values and values outside the documented interval.
    pub fn new(concentration: f64) -> Result<Self> {
        if concentration.is_nan() || concentration.is_infinite() {
            return Err(MetisError::clinical(
                ErrorCode::NumericInstability,
                "Drug concentration cannot be NaN or Infinite",
            ));
        }
        if !(Self::MIN_CONC..=Self::MAX_CONC).contains(&concentration) {
            return Err(MetisError::clinical(
                ErrorCode::InvalidDrugConcentration,
                format!(
                    "Concentration {} mg/mL outside demonstration bounds [{}, {}]",
                    concentration,
                    Self::MIN_CONC,
                    Self::MAX_CONC
                ),
            ));
        }
        Ok(Self(concentration))
    }

    /// Returns the numerical concentration value.
    #[must_use]
    pub const fn get(&self) -> f64 {
        self.0
    }
}

/// Target dosage rate in micrograms per kilogram per minute (mcg/kg/min).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TargetDoseRate(f64);

impl TargetDoseRate {
    /// Minimum dose admitted by this demonstration.
    pub const MIN_DOSE: f64 = 0.0001;
    /// Maximum dose admitted by this demonstration.
    pub const MAX_DOSE: f64 = 100.0;

    /// Creates a validated `TargetDoseRate` or returns `MetisError`.
    ///
    /// # Errors
    /// Rejects nonfinite values and values outside the documented interval.
    pub fn new(dose: f64) -> Result<Self> {
        if dose.is_nan() || dose.is_infinite() {
            return Err(MetisError::clinical(
                ErrorCode::NumericInstability,
                "Target dose cannot be NaN or Infinite",
            ));
        }
        if !(Self::MIN_DOSE..=Self::MAX_DOSE).contains(&dose) {
            return Err(MetisError::clinical(
                ErrorCode::InvalidTargetDose,
                format!(
                    "Dose {} mcg/kg/min outside demonstration bounds [{}, {}]",
                    dose,
                    Self::MIN_DOSE,
                    Self::MAX_DOSE
                ),
            ));
        }
        Ok(Self(dose))
    }

    /// Returns the target dose value.
    #[must_use]
    pub const fn get(&self) -> f64 {
        self.0
    }
}

/// Clinical safety envelope parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SafetyEnvelope {
    /// Maximum allowable infusion rate in mL/hour for adults.
    max_adult_rate_ml_hr: f64,
    /// Maximum allowable infusion rate in mL/hour for pediatric patients.
    max_pediatric_rate_ml_hr: f64,
    /// Patient weight threshold (kg) below which pediatric rules apply.
    pediatric_weight_threshold_kg: f64,
}

impl SafetyEnvelope {
    /// Validates demonstration limits in mL/hour and a weight threshold in kg.
    ///
    /// # Errors
    /// Rejects nonfinite or nonpositive ceilings, a pediatric ceiling above the
    /// adult ceiling, and thresholds outside the supported weight interval.
    ///
    /// # Examples
    /// ```
    /// let envelope = metis_backend::clinical::SafetyEnvelope::new(300.0, 50.0, 35.0)?;
    /// # Ok::<(), metis_core::error::MetisError>(())
    /// ```
    pub fn new(adult: f64, pediatric: f64, threshold: f64) -> Result<Self> {
        if !adult.is_finite()
            || !pediatric.is_finite()
            || !threshold.is_finite()
            || adult <= 0.0
            || pediatric <= 0.0
            || pediatric > adult
            || !(PatientWeightKg::MIN_WEIGHT_KG..=PatientWeightKg::MAX_WEIGHT_KG)
                .contains(&threshold)
        {
            return Err(MetisError::clinical(
                ErrorCode::ClinicalInterlockBlocked,
                "Envelope requires finite positive ordered ceilings and a supported weight threshold",
            ));
        }
        Ok(Self {
            max_adult_rate_ml_hr: adult,
            max_pediatric_rate_ml_hr: pediatric,
            pediatric_weight_threshold_kg: threshold,
        })
    }
}

impl Default for SafetyEnvelope {
    fn default() -> Self {
        Self {
            max_adult_rate_ml_hr: 300.0,
            max_pediatric_rate_ml_hr: 50.0,
            pediatric_weight_threshold_kg: 35.0,
        }
    }
}

/// Calculated infusion rate with clinical approval status.
#[derive(Debug, Clone, PartialEq)]
pub struct ClinicalCalculationResult {
    /// Calculated infusion rate in mL/hour.
    pub rate_ml_hr: f64,
    /// Total drug delivery rate in milligrams per hour.
    pub drug_rate_mg_hr: f64,
    /// Whether pediatric safety limits were enforced.
    pub is_pediatric: bool,
}

/// Evaluates a controlled infusion rate calculation under a strict safety envelope.
///
/// Formula:
/// `Rate (mL/hr) = (Dose (mcg/kg/min) * Weight (kg) * 60 min/hr) / (Concentration (mg/mL) * 1000 mcg/mg)`
///
/// The factor 60 converts minutes to hours and 1000 converts mg to mcg.
/// The boundary is inclusive: a rate equal to its ceiling succeeds. The pediatric
/// limit applies strictly below the configured weight threshold.
///
/// # Errors
/// Rejects nonfinite output and rates above the applicable ceiling.
pub fn calculate_infusion_rate(
    weight: PatientWeightKg,
    concentration: DrugConcentrationMgMl,
    dose: TargetDoseRate,
    envelope: &SafetyEnvelope,
) -> Result<ClinicalCalculationResult> {
    let w = weight.get();
    let c = concentration.get();
    let d = dose.get();

    // Standard clinical rate formula
    let rate_ml_hr = (d * w * 60.0) / (c * 1000.0);

    if rate_ml_hr.is_nan() || rate_ml_hr.is_infinite() {
        return Err(MetisError::clinical(
            ErrorCode::NumericInstability,
            "Calculation produced non-finite infusion rate",
        ));
    }

    let is_pediatric = w < envelope.pediatric_weight_threshold_kg;
    let max_allowed = if is_pediatric {
        envelope.max_pediatric_rate_ml_hr
    } else {
        envelope.max_adult_rate_ml_hr
    };

    if rate_ml_hr > max_allowed {
        let err_code = if is_pediatric {
            ErrorCode::PediatricRateExceeded
        } else {
            ErrorCode::RateExceedsSafetyEnvelope
        };
        return Err(MetisError::clinical(
            err_code,
            format!(
                "Calculated rate {rate_ml_hr:.3} mL/hr exceeds configured demonstration limit {max_allowed:.3} mL/hr"
            ),
        ));
    }

    let drug_rate_mg_hr = rate_ml_hr * c;

    Ok(ClinicalCalculationResult {
        rate_ml_hr,
        drug_rate_mg_hr,
        is_pediatric,
    })
}
