use super::{
    PageMessage, PermissionProbeAction, PermissionProbeComplete, WebViewAction, WebViewRequest,
    is_permission_probe_complete,
};
use metis_backend::clinical::{
    DrugConcentrationMgMl, PatientWeightKg, SafetyEnvelope, TargetDoseRate, calculate_infusion_rate,
};
use metis_core::error::ErrorCode;
use serde_json::{Value, json};

fn capture() -> tracing::subscriber::DefaultGuard {
    tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_test_writer()
            .without_time()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .finish(),
    )
}

/// These vectors produce exact binary64 results by the published rate formula:
/// 0.5 * 62.5 * 60 / (1.875 * 1000) = 1, and half that weight gives 0.5.
/// Drug rates are respectively 1.875 and 0.9375; 31.25 kg is pediatric.
fn calculated_message(audit_sequence_id: u64, weight: f64) -> PageMessage {
    let result = calculate_infusion_rate(
        PatientWeightKg::new(weight).expect("valid vector weight"),
        DrugConcentrationMgMl::new(1.875).expect("valid vector concentration"),
        TargetDoseRate::new(0.5).expect("valid vector dose"),
        &SafetyEnvelope::default(),
    )
    .expect("analytically admitted rate");
    PageMessage::Result {
        status: "success",
        audit_sequence_id,
        rate_ml_hr: result.rate_ml_hr,
        drug_rate_mg_hr: result.drug_rate_mg_hr,
        is_pediatric: result.is_pediatric,
    }
}

#[test]
fn result_objects_preserve_audit_boundaries_and_computed_rates() {
    let _trace = capture();
    let _test = tracing::info_span!("page_result_object_boundaries").entered();
    let cases = tracing::info_span!("arrange").in_scope(|| {
        [
            (1, "1"),
            ((1 << 53) - 1, "9007199254740991"),
            (1 << 53, "9007199254740992"),
            ((1 << 53) + 1, "9007199254740993"),
            (u64::MAX - 1, "18446744073709551614"),
            (u64::MAX, "18446744073709551615"),
        ]
    });
    let clinical_cases: [(f64, f64, f64, bool); 2] = tracing::info_span!("arrange", case = "rates")
        .in_scope(|| [(62.5, 1.0, 1.875, false), (31.25, 0.5, 0.9375, true)]);
    for (audit, digits) in cases {
        for (weight, rate, drug, pediatric) in clinical_cases {
            let (message, object, encoded, decoded) = tracing::info_span!("act", audit, weight)
                .in_scope(|| {
                    let message = calculated_message(audit, weight);
                    let object = serde_json::to_value(&message).expect("page object");
                    let encoded = serde_json::to_string(&message).expect("encoded page object");
                    let decoded: Value =
                        serde_json::from_str(&encoded).expect("decoded page object");
                    (message, object, encoded, decoded)
                });
            let _assert = tracing::info_span!("assert", audit, weight).entered();
            let expected = json!({
                "type": "result",
                "status": "success",
                "audit_sequence_id": digits,
                "rate_ml_hr": rate,
                "drug_rate_mg_hr": drug,
                "is_pediatric": pediatric,
            });
            assert_eq!(object, expected);
            assert_eq!(decoded, expected);
            assert!(encoded.contains(&format!("\"audit_sequence_id\":\"{digits}\"")));
            let received = decoded["audit_sequence_id"]
                .as_str()
                .expect("decimal string");
            assert_eq!(received, digits);
            assert_eq!(received.parse::<u64>().expect("full-range integer"), audit);
            let PageMessage::Result {
                audit_sequence_id,
                rate_ml_hr,
                drug_rate_mg_hr,
                is_pediatric,
                ..
            } = message
            else {
                panic!("calculated result variant")
            };
            assert_eq!(audit_sequence_id, audit);
            assert_eq!(rate_ml_hr.to_bits(), rate.to_bits());
            assert_eq!(drug_rate_mg_hr.to_bits(), drug.to_bits());
            assert_eq!(is_pediatric, pediatric);
        }
    }
}

#[test]
fn zero_is_a_serialization_boundary_without_claiming_a_backend_audit() {
    let _trace = capture();
    let _test = tracing::info_span!("page_result_adversarial_zero").entered();
    // AuditLedger starts at one. This exercises the serializer's entire u64
    // representation domain; it does not assert that the backend issues zero.
    let message = tracing::info_span!("arrange").in_scope(|| calculated_message(0, 62.5));
    let object = tracing::info_span!("act")
        .in_scope(|| serde_json::to_value(&message).expect("zero-boundary object"));
    let _assert = tracing::info_span!("assert").entered();
    assert_eq!(
        object,
        json!({"type": "result", "status": "success", "audit_sequence_id": "0",
               "rate_ml_hr": 1.0, "drug_rate_mg_hr": 1.875, "is_pediatric": false})
    );
    assert_eq!(
        object["audit_sequence_id"]
            .as_str()
            .expect("zero text")
            .parse::<u64>()
            .expect("zero integer"),
        0
    );
}

#[test]
fn error_object_preserves_actual_clinical_rejection() {
    let _trace = capture();
    let _test = tracing::info_span!("page_clinical_error_object").entered();
    let error = tracing::info_span!("arrange").in_scope(|| {
        PatientWeightKg::new(PatientWeightKg::MIN_WEIGHT_KG / 2.0)
            .expect_err("below the analytical lower bound")
    });
    tracing::info_span!("assert", case = "actual_rejection").in_scope(|| {
        assert_eq!(error.code, ErrorCode::InvalidPatientWeight);
    });
    let object = tracing::info_span!("act").in_scope(|| {
        serde_json::to_value(PageMessage::Error {
            status: "rejected",
            // Published InvalidPatientWeight wire code; the actual typed error
            // above and retained message independently establish this vector.
            error_code: 0x3001,
            message: error.message,
        })
        .expect("clinical error object")
    });
    let _assert = tracing::info_span!("assert").entered();
    assert_eq!(
        object,
        json!({"type": "error", "status": "rejected", "error_code": 0x3001,
               "message": "Weight 0.125 kg outside demonstration bounds [0.25, 500]"})
    );
}

#[test]
fn permission_objects_preserve_all_declared_kinds_and_initiation() {
    let _trace = capture();
    let _test = tracing::info_span!("page_permission_objects").entered();
    let permissions = tracing::info_span!("arrange")
        .in_scope(|| ["geolocation", "camera", "microphone", "notifications"]);
    let wire_code = 0x200f;
    tracing::info_span!("assert", case = "published_permission_code").in_scope(|| {
        assert_eq!(
            ErrorCode::from_wire(wire_code),
            Some(ErrorCode::PermissionDenied)
        );
    });
    for permission in permissions {
        for user_initiated in [false, true] {
            let object = tracing::info_span!("act", permission, user_initiated).in_scope(|| {
                serde_json::to_value(PageMessage::PermissionDenied {
                    status: "permission_denied",
                    error_code: wire_code,
                    permission: permission.to_owned(),
                    message: format!("WebView2 denied {permission} access request"),
                    user_initiated,
                })
                .expect("permission object")
            });
            let _assert = tracing::info_span!("assert", permission, user_initiated).entered();
            assert_eq!(
                object,
                json!({"type": "permission_denied", "status": "permission_denied",
                       "error_code": 0x200f, "permission": permission,
                       "message": format!("WebView2 denied {permission} access request"),
                       "user_initiated": user_initiated})
            );
        }
    }
}

#[test]
fn request_parser_rejects_unknown_fields_and_preserves_inputs() {
    let _trace = capture();
    let _test = tracing::info_span!("page_request_parser").entered();
    let input = tracing::info_span!("arrange").in_scope(|| {
        r#"{"action":"submit","patient_id":"PT-1","weight_kg":72.5,"concentration_mg_ml":4.0,"target_dose_mcg_kg_min":0.5}"#
    });
    let request: WebViewRequest = tracing::info_span!("act")
        .in_scope(|| serde_json::from_str(input).expect("bounded request"));
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(request.action, WebViewAction::Submit);
        assert_eq!(request.patient_id, "PT-1");
        assert_eq!(request.weight_kg.to_bits(), 72.5_f64.to_bits());
        assert_eq!(request.concentration_mg_ml.to_bits(), 4.0_f64.to_bits());
        assert_eq!(request.target_dose_mcg_kg_min.to_bits(), 0.5_f64.to_bits());
    });
    let malformed = tracing::info_span!("arrange", case = "unknown_field")
        .in_scope(|| input.replace("\"action\":", "\"extra\":true,\"action\":"));
    let error = tracing::info_span!("act", case = "unknown_field").in_scope(|| {
        serde_json::from_str::<WebViewRequest>(&malformed).expect_err("unknown field")
    });
    let _assert = tracing::info_span!("assert", case = "unknown_field").entered();
    assert_eq!(error.classify(), serde_json::error::Category::Data);
    assert!(error.to_string().contains("unknown field `extra`"));
}

#[test]
fn permission_capture_waits_for_typed_completion_message() {
    let _trace = capture();
    let _test = tracing::info_span!("page_permission_completion").entered();
    let cases = tracing::info_span!("arrange").in_scope(|| {
        [
            (r#"{"action":"permission_probe_complete"}"#, true),
            (
                r#"{"action":"permission_probe_complete","extra":true}"#,
                false,
            ),
            (r#"{"action":"submit"}"#, false),
        ]
    });
    for (input, expected) in cases {
        let (complete, decoded) = tracing::info_span!("act", input).in_scope(|| {
            (
                is_permission_probe_complete(input),
                serde_json::from_str::<PermissionProbeComplete>(input),
            )
        });
        let _assert = tracing::info_span!("assert", input).entered();
        assert_eq!(complete, expected);
        match decoded {
            Ok(message) => assert_eq!(message.action, PermissionProbeAction::Complete),
            Err(error) => assert_eq!(error.classify(), serde_json::error::Category::Data),
        }
    }
}
