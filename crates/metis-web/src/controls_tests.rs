use super::{
    ControlField, ControlState, DisplayUnit, FormInputs, FormState, InputField, ResultDetail,
    ScalePercent, Theme, change_control_field, input_control_field, input_field, update_control,
};
use metis_core::protocol::ClinicalCalcResponsePayload;

#[test]
fn control_state_applies_checked_unit_and_scale_changes() {
    let mut controls = ControlState::default();
    assert!(controls.show_events());
    assert_eq!(controls.display_unit(), DisplayUnit::Volume);
    assert_eq!(controls.scale().value(), 100);
    assert_eq!(controls.result_detail(), ResultDetail::Summary);
    assert_eq!(controls.theme(), Theme::System);

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
    assert!(controls.apply(ControlField::Theme, None, Some("dark")));
    assert_eq!(controls.theme(), Theme::Dark);
    assert_eq!(
        controls.summary(),
        "View options: events hidden; drug mass rate; detail audit detail; scale 150%; theme dark"
    );
}

#[test]
fn delegated_bindings_cover_each_control_event() {
    assert!(matches!(
        input_field("patient-id"),
        Some(InputField::Patient)
    ));
    assert!(matches!(input_field("weight-kg"), Some(InputField::Weight)));
    assert!(matches!(
        input_field("concentration-mg-ml"),
        Some(InputField::Concentration)
    ));
    assert!(matches!(input_field("target-dose"), Some(InputField::Dose)));
    assert!(matches!(
        input_control_field("result-scale"),
        Some(ControlField::Scale)
    ));
    assert!(matches!(
        change_control_field("show-events"),
        Some(ControlField::ShowEvents)
    ));
    assert!(matches!(
        change_control_field("dose-volume"),
        Some(ControlField::DisplayUnit(DisplayUnit::Volume))
    ));
    assert!(matches!(
        change_control_field("dose-mass"),
        Some(ControlField::DisplayUnit(DisplayUnit::DrugMass))
    ));
    assert!(matches!(
        change_control_field("result-detail-select"),
        Some(ControlField::ResultDetail)
    ));
    assert!(matches!(
        change_control_field("theme-mode"),
        Some(ControlField::Theme)
    ));
    assert!(input_field("unknown").is_none());
    assert!(input_control_field("unknown").is_none());
    assert!(change_control_field("result-scale").is_none());
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
    assert!(!controls.apply(ControlField::Theme, None, Some("sepia")));
    assert_eq!(controls.display_unit(), DisplayUnit::Volume);
    assert_eq!(controls.scale().value(), 100);
    assert_eq!(controls.result_detail(), ResultDetail::Summary);
    assert_eq!(controls.theme(), Theme::System);
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
    update_control(
        &mut controls,
        &mut state,
        ControlField::Theme,
        None,
        Some("light"),
    );
    assert_eq!(controls.theme(), Theme::Light);
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
