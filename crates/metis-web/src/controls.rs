use metis_core::error::{ErrorCode, MetisError};
use metis_frontend::{FormInputs, FormState};

#[cfg(target_arch = "wasm32")]
pub(crate) const BROWSER_MARKUP: &str = r#"
<header class="metis-header">
  <p class="metis-kicker">METIS / BROWSER WORKBENCH</p>
  <h1>Authorized clinical form boundary</h1>
  <p id="metis-status" role="status">Browser controls are active.</p>
  <p id="metis-capabilities">Host capabilities: unavailable</p>
  <p id="metis-plugins">Registered frontend extensions: unavailable</p>
  <p id="metis-events" role="status">Remote events: none</p>
  <button id="open-session-dialog" type="button" aria-haspopup="dialog" aria-controls="session-dialog">Session details</button>
</header>
<dialog id="session-dialog" aria-labelledby="session-dialog-heading">
  <h2 id="session-dialog-heading">Authorized session details</h2>
  <p id="session-dialog-status" role="status">Controls active; no authorized backend bridge configured</p>
  <p id="session-dialog-capabilities">Host capabilities: unavailable</p>
  <button id="session-dialog-close" type="button">Close</button>
</dialog>
<form id="metis-form" class="metis-form">
  <label for="patient-id">Patient reference</label>
  <input id="patient-id" name="patient-id" value="PT-9042-ALPHA" autocomplete="off">
  <label for="weight-kg">Weight (kg)</label>
  <input id="weight-kg" name="weight-kg" type="number" step="any" value="72.5">
  <label for="concentration-mg-ml">Drug concentration (mg/mL)</label>
  <input id="concentration-mg-ml" name="concentration-mg-ml" type="number" step="any" value="4">
  <label for="target-dose">Target dose (mcg/kg/min)</label>
  <input id="target-dose" name="target-dose" type="number" step="any" value="0.5">
  <button id="submit-calculation" type="submit">Submit to authorized backend</button>
</form>
<fieldset id="view-options" class="metis-options">
  <legend>View options</legend>
  <label class="metis-option" for="show-events">
    <input id="show-events" type="checkbox" checked>
    Show remote events
  </label>
  <span class="metis-option-label">Display unit</span>
  <label class="metis-option" for="dose-volume">
    <input id="dose-volume" name="display-unit" type="radio" value="volume" checked>
    Volume rate
  </label>
  <label class="metis-option" for="dose-mass">
    <input id="dose-mass" name="display-unit" type="radio" value="mass">
    Drug mass rate
  </label>
  <label for="result-scale">Result scale</label>
  <input id="result-scale" type="range" min="50" max="150" step="10" value="100">
  <label for="result-detail-select">Result detail</label>
  <select id="result-detail-select" name="result-detail">
    <option value="summary" selected>Clinical summary</option>
    <option value="audit">Audit detail</option>
  </select>
  <p id="options-state" role="status">View options: events visible; volume rate; detail clinical summary; scale 100%</p>
</fieldset>
<section class="metis-pointer" aria-labelledby="pointer-heading">
  <h2 id="pointer-heading">Pointer capture</h2>
  <p id="pointer-status" role="status">Pointer capture: idle</p>
  <p id="wheel-status" role="status">Wheel: idle</p>
  <p id="gesture-status" role="status">Gesture: idle; pan (0.0, 0.0) CSS px; zoom 100%</p>
  <div id="pointer-surface" role="group" tabindex="0" aria-label="Pointer capture surface">
    <div id="gesture-content">Press or drag this surface to exercise Rust-owned pointer capture, metadata, wheel pan and Ctrl+wheel zoom.</div>
  </div>
</section>
<section class="metis-drop" aria-labelledby="drop-heading">
  <h2 id="drop-heading">DICOM file drop</h2>
  <p id="drop-status" role="status">Drop status: ready; no files captured</p>
  <div id="drop-zone" role="group" tabindex="0" aria-describedby="drop-status" aria-label="DICOM file drop zone" data-drop-state="idle" data-drop-count="0">
    <p>Drop DICOM files here to inspect bounded metadata. File bytes remain with the host.</p>
  </div>
</section>
<section class="metis-result" aria-labelledby="result-heading">
  <h2 id="result-heading">Backend result</h2>
  <p id="result-state">No backend bridge configured.</p>
  <p id="result-metrics">Volume rate: unavailable</p>
  <p id="result-detail">Clinical summary awaiting backend response</p>
  <dl>
    <dt>Patient</dt><dd id="result-patient">PT-9042-ALPHA</dd>
    <dt>Weight</dt><dd id="result-weight">72.50 kg</dd>
    <dt>Concentration</dt><dd id="result-concentration">4.00 mg/mL</dd>
    <dt>Dose</dt><dd id="result-dose">0.500 mcg/kg/min</dd>
  </dl>
</section>
"#;

#[derive(Clone, Copy)]
pub(crate) enum InputField {
    Patient,
    Weight,
    Concentration,
    Dose,
}

pub(crate) fn update_input(
    inputs: &mut FormInputs,
    form_state: &mut FormState,
    field: InputField,
    value: &str,
) {
    match field {
        InputField::Patient => value.clone_into(&mut inputs.patient_id),
        InputField::Weight => {
            let Some(value) = parse_finite(value) else {
                *form_state = invalid_input("weight");
                return;
            };
            inputs.weight_kg = value;
        }
        InputField::Concentration => {
            let Some(value) = parse_finite(value) else {
                *form_state = invalid_input("concentration");
                return;
            };
            inputs.concentration_mg_ml = value;
        }
        InputField::Dose => {
            let Some(value) = parse_finite(value) else {
                *form_state = invalid_input("dose");
                return;
            };
            inputs.target_dose_mcg_kg_min = value;
        }
    }
    *form_state = FormState::Idle;
}

#[derive(Clone, Copy)]
pub(crate) enum ControlField {
    ShowEvents,
    DisplayUnit(DisplayUnit),
    Scale,
    ResultDetail,
}

impl ControlField {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::ShowEvents => "show events",
            Self::DisplayUnit(_) => "display unit",
            Self::Scale => "result scale",
            Self::ResultDetail => "result detail",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DisplayUnit {
    Volume,
    DrugMass,
}

impl DisplayUnit {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Volume => "volume rate",
            Self::DrugMass => "drug mass rate",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResultDetail {
    Summary,
    Audit,
}

impl ResultDetail {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "summary" => Some(Self::Summary),
            "audit" => Some(Self::Audit),
            _ => None,
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Summary => "clinical summary",
            Self::Audit => "audit detail",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScalePercent(u16);

impl ScalePercent {
    const MIN: u16 = 50;
    const MAX: u16 = 150;
    const STEP: u16 = 10;
    const DEFAULT: Self = Self(100);

    pub(crate) fn parse(value: &str) -> Option<Self> {
        let value = value.parse::<u16>().ok()?;
        if !(Self::MIN..=Self::MAX).contains(&value)
            || !(value - Self::MIN).is_multiple_of(Self::STEP)
        {
            return None;
        }
        Some(Self(value))
    }

    pub(crate) const fn value(self) -> u16 {
        self.0
    }
}

impl Default for ScalePercent {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone)]
pub(crate) struct ControlState {
    show_events: bool,
    display_unit: DisplayUnit,
    scale: ScalePercent,
    result_detail: ResultDetail,
}

impl Default for ControlState {
    fn default() -> Self {
        Self {
            show_events: true,
            display_unit: DisplayUnit::Volume,
            scale: ScalePercent::default(),
            result_detail: ResultDetail::Summary,
        }
    }
}

impl ControlState {
    pub(crate) fn apply(
        &mut self,
        field: ControlField,
        checked: Option<bool>,
        value: Option<&str>,
    ) -> bool {
        match field {
            ControlField::ShowEvents => {
                let Some(checked) = checked else {
                    return false;
                };
                self.show_events = checked;
            }
            ControlField::DisplayUnit(unit) => {
                if checked != Some(true) {
                    return false;
                }
                self.display_unit = unit;
            }
            ControlField::Scale => {
                let Some(scale) = value.and_then(ScalePercent::parse) else {
                    return false;
                };
                self.scale = scale;
            }
            ControlField::ResultDetail => {
                let Some(detail) = value.and_then(ResultDetail::parse) else {
                    return false;
                };
                self.result_detail = detail;
            }
        }
        true
    }

    pub(crate) const fn show_events(&self) -> bool {
        self.show_events
    }

    pub(crate) const fn display_unit(&self) -> DisplayUnit {
        self.display_unit
    }

    pub(crate) const fn scale(&self) -> ScalePercent {
        self.scale
    }

    pub(crate) const fn result_detail(&self) -> ResultDetail {
        self.result_detail
    }

    pub(crate) fn summary(&self) -> String {
        let event_visibility = if self.show_events {
            "events visible"
        } else {
            "events hidden"
        };
        format!(
            "View options: {event_visibility}; {}; detail {}; scale {}%",
            self.display_unit.label(),
            self.result_detail.label(),
            self.scale.value(),
        )
    }
}

pub(crate) fn update_control(
    controls: &mut ControlState,
    form_state: &mut FormState,
    field: ControlField,
    checked: Option<bool>,
    value: Option<&str>,
) {
    if !controls.apply(field, checked, value) {
        *form_state = invalid_control(field.name());
    }
}

fn parse_finite(value: &str) -> Option<f64> {
    value.parse::<f64>().ok().filter(|value| value.is_finite())
}

fn invalid_input(field: &str) -> FormState {
    FormState::Failed(MetisError::clinical(
        ErrorCode::NumericInstability,
        format!("Browser field {field} must contain a finite number"),
    ))
}

pub(crate) fn invalid_control(field: &str) -> FormState {
    FormState::Failed(MetisError::clinical(
        ErrorCode::NumericInstability,
        format!("Browser control {field} contains an invalid value"),
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        ControlField, ControlState, DisplayUnit, FormInputs, FormState, ResultDetail, ScalePercent,
        update_control,
    };
    use metis_core::protocol::ClinicalCalcResponsePayload;

    #[test]
    fn control_state_applies_checked_unit_and_scale_changes() {
        let mut controls = ControlState::default();
        assert!(controls.show_events());
        assert_eq!(controls.display_unit(), DisplayUnit::Volume);
        assert_eq!(controls.scale().value(), 100);
        assert_eq!(controls.result_detail(), ResultDetail::Summary);

        assert!(controls.apply(ControlField::ShowEvents, Some(false), None));
        assert!(!controls.show_events());
        assert!(controls.apply(
            ControlField::DisplayUnit(DisplayUnit::DrugMass),
            Some(true),
            Some("mass"),
        ));
        assert_eq!(controls.display_unit(), DisplayUnit::DrugMass);
        assert!(controls.apply(ControlField::Scale, None, Some("150")));
        assert_eq!(controls.scale().value(), 150);
        assert!(controls.apply(ControlField::ResultDetail, None, Some("audit")));
        assert_eq!(controls.result_detail(), ResultDetail::Audit);
        assert_eq!(
            controls.summary(),
            "View options: events hidden; drug mass rate; detail audit detail; scale 150%"
        );
    }

    #[test]
    fn control_state_rejects_invalid_checked_and_scale_values() {
        let mut controls = ControlState::default();
        assert!(!controls.apply(ControlField::ShowEvents, None, None));
        assert!(!controls.apply(
            ControlField::DisplayUnit(DisplayUnit::DrugMass),
            Some(false),
            Some("mass"),
        ));
        assert!(!controls.apply(ControlField::Scale, None, Some("151")));
        assert!(!controls.apply(ControlField::ResultDetail, None, Some("other")));
        assert_eq!(controls.display_unit(), DisplayUnit::Volume);
        assert_eq!(controls.scale().value(), 100);
        assert_eq!(controls.result_detail(), ResultDetail::Summary);
    }

    #[test]
    fn view_control_updates_preserve_a_backend_result() {
        let response = ClinicalCalcResponsePayload {
            audit_sequence_id: 4,
            rate_ml_hr: 0.54375,
            drug_rate_mg_hr: 2.175,
            is_pediatric: false,
            result_signature: [0; 32],
        };
        let mut controls = ControlState::default();
        let mut state = FormState::Success(response.clone());

        update_control(
            &mut controls,
            &mut state,
            ControlField::ShowEvents,
            Some(false),
            None,
        );
        update_control(
            &mut controls,
            &mut state,
            ControlField::DisplayUnit(DisplayUnit::DrugMass),
            Some(true),
            Some("mass"),
        );
        update_control(
            &mut controls,
            &mut state,
            ControlField::Scale,
            None,
            Some("120"),
        );
        update_control(
            &mut controls,
            &mut state,
            ControlField::ResultDetail,
            None,
            Some("audit"),
        );

        assert_eq!(state, FormState::Success(response));
        assert!(!controls.show_events());
        assert_eq!(controls.display_unit(), DisplayUnit::DrugMass);
        assert_eq!(controls.scale().value(), 120);
        assert_eq!(controls.result_detail(), ResultDetail::Audit);
    }

    #[test]
    fn numeric_input_update_rejects_non_finite_values() {
        let mut inputs = FormInputs::new("PT-1", 70.0, 4.0, 0.5);
        let mut state = FormState::Idle;
        super::update_input(&mut inputs, &mut state, super::InputField::Weight, "NaN");
        assert!(matches!(state, FormState::Failed(_)));
        assert_eq!(inputs.weight_kg.to_bits(), 70.0_f64.to_bits());
    }

    #[test]
    fn numeric_input_update_preserves_each_declared_field() {
        let mut inputs = FormInputs::new("PT-1", 70.0, 4.0, 0.5);
        let mut state = FormState::Idle;
        super::update_input(&mut inputs, &mut state, super::InputField::Patient, "PT-2");
        super::update_input(
            &mut inputs,
            &mut state,
            super::InputField::Concentration,
            "5.0",
        );
        super::update_input(&mut inputs, &mut state, super::InputField::Dose, "0.75");
        assert_eq!(inputs.patient_id, "PT-2");
        assert_eq!(inputs.concentration_mg_ml.to_bits(), 5.0_f64.to_bits());
        assert_eq!(inputs.target_dose_mcg_kg_min.to_bits(), 0.75_f64.to_bits());
    }

    #[test]
    fn scale_parser_accepts_the_declared_range_only() {
        assert_eq!(ScalePercent::parse("50").map(ScalePercent::value), Some(50));
        assert_eq!(
            ScalePercent::parse("150").map(ScalePercent::value),
            Some(150)
        );
        assert!(ScalePercent::parse("49").is_none());
        assert!(ScalePercent::parse("51").is_none());
        assert!(ScalePercent::parse("151").is_none());
        assert!(ScalePercent::parse("not-a-number").is_none());
    }
}
