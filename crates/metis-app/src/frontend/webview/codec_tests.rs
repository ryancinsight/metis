//! Portable native-consumer regression vectors through the shared page codec.

use metis_backend::clinical::{
    DrugConcentrationMgMl, PatientWeightKg, SafetyEnvelope, TargetDoseRate, calculate_infusion_rate,
};
use metis_core::{
    embedded_assets::Digest, error::ErrorCode, protocol::ClinicalCalcResponsePayload,
};
use metis_frontend::FormInputs;
use metis_web::embedded::{
    BrowserPermissionResult, CapabilityObservation, PageCapability, PageCommand, PageEvent,
    PageIdentity, PageRole, ResourceCounts, SequenceId, decode_page_command, decode_page_event,
    encode_page_command, encode_page_event,
};
use serde_json::{Value, json};

const BYTE_BUDGET: usize = 64 * 1024;
const MODULE_DIGEST: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn capture() -> tracing::subscriber::DefaultGuard {
    tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_test_writer()
            .without_time()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .finish(),
    )
}

fn identity(role: PageRole) -> PageIdentity {
    PageIdentity::new(
        role,
        SequenceId::new(1).expect("positive mount"),
        Digest::hash(b"abc"),
    )
}

fn wire_identity(role: &str) -> Value {
    json!({"schema":1,"role":role,"mount_id":"1","module_digest":MODULE_DIGEST})
}

/// Independent dyadic model: .5*62.5*60/(1.875*1000)=1, and half the
/// weight gives .5. Drug rates are 1.875 and .9375; 31.25 kg is pediatric.
fn calculated(audit_sequence_id: u64, weight: f64) -> ClinicalCalcResponsePayload {
    let rate = calculate_infusion_rate(
        PatientWeightKg::new(weight).expect("vector weight"),
        DrugConcentrationMgMl::new(1.875).expect("vector concentration"),
        TargetDoseRate::new(0.5).expect("vector dose"),
        &SafetyEnvelope::default(),
    )
    .expect("analytically admitted calculation");
    ClinicalCalcResponsePayload {
        audit_sequence_id,
        rate_ml_hr: rate.rate_ml_hr,
        drug_rate_mg_hr: rate.drug_rate_mg_hr,
        is_pediatric: rate.is_pediatric,
        // Published SHA256 bytes exercise opaque signature-field preservation.
        // This serialization test makes no backend MAC-attestation claim.
        result_signature: Digest::hash(b"abc").into_bytes(),
    }
}

#[test]
fn shared_result_codec_preserves_full_audit_domain_and_real_dyadic_rates() {
    let _trace = capture();
    let _test = tracing::info_span!("native_shared_result_boundary").entered();
    let audits = tracing::info_span!("arrange").in_scope(|| {
        [
            (1, "1"),
            ((1 << 53) - 1, "9007199254740991"),
            (1 << 53, "9007199254740992"),
            ((1 << 53) + 1, "9007199254740993"),
            (u64::MAX - 1, "18446744073709551614"),
            (u64::MAX, "18446744073709551615"),
        ]
    });
    let clinical: [(f64, f64, f64, bool, &str, &str); 2] = [
        (62.5, 1.0, 1.875, false, "1.0", "1.875"),
        (31.25, 0.5, 0.9375, true, "0.5", "0.9375"),
    ];
    for (audit, digits) in audits {
        for (weight, rate, drug, pediatric, rate_decimal, drug_decimal) in clinical {
            let (command, encoded, decoded, object) = tracing::info_span!("act", audit, weight)
                .in_scope(|| {
                    let command = PageCommand::Result {
                        identity: identity(PageRole::Form),
                        request: SequenceId::new(1).expect("positive request"),
                        response: calculated(audit, weight),
                    };
                    let encoded = encode_page_command(&command, BYTE_BUDGET).expect("shared codec");
                    let decoded =
                        decode_page_command(&encoded, BYTE_BUDGET).expect("shared decoded result");
                    let object: Value = serde_json::from_str(&encoded).expect("object value");
                    (command, encoded, decoded, object)
                });
            tracing::info_span!("assert",audit,weight).in_scope(|| {
                assert_eq!(command,decoded);
                assert_eq!(object,json!({"type":"result","identity":wire_identity("form"),"request_id":"1",
                    "audit_sequence_id":digits,"rate_ml_hr":rate_decimal,"drug_rate_mg_hr":drug_decimal,"is_pediatric":pediatric,
                    "result_signature":Digest::hash(b"abc").into_bytes()}));
                assert_eq!(object["audit_sequence_id"].as_str().expect("decimal text").parse::<u64>().expect("exact integer"),audit);
                let PageCommand::Result { response,.. } = decoded else { panic!("result command"); };
                assert_eq!(response.rate_ml_hr.to_bits(),rate.to_bits());
                assert_eq!(response.drug_rate_mg_hr.to_bits(),drug.to_bits());
                assert_eq!(response.is_pediatric,pediatric);
                assert!(encoded.contains(&format!("\"audit_sequence_id\":\"{digits}\"")));
            });
        }
    }
}

#[test]
fn zero_audit_is_a_wire_boundary_without_claiming_backend_issuance() {
    let _trace = capture();
    let _test = tracing::info_span!("native_shared_zero_audit").entered();
    let command = tracing::info_span!("arrange").in_scope(|| PageCommand::Result {
        identity: identity(PageRole::Form),
        request: SequenceId::new(1).expect("positive request"),
        response: calculated(0, 62.5),
    });
    let wire = tracing::info_span!("act")
        .in_scope(|| encode_page_command(&command, BYTE_BUDGET).expect("full-domain codec"));
    tracing::info_span!("assert").in_scope(|| {
        let value: Value = serde_json::from_str(&wire).expect("zero-boundary value");
        assert_eq!(value["audit_sequence_id"], "0");
        assert_eq!(
            decode_page_command(&wire, BYTE_BUDGET).expect("zero preserved"),
            command
        );
        assert_eq!(value["rate_ml_hr"], "1.0");
        assert_eq!(value["drug_rate_mg_hr"], "1.875");
    });
}

#[test]
fn shared_error_codec_preserves_real_clinical_rejection_context() {
    let _trace = capture();
    let _test = tracing::info_span!("native_shared_clinical_error").entered();
    let failure = tracing::info_span!("arrange").in_scope(|| {
        PatientWeightKg::new(PatientWeightKg::MIN_WEIGHT_KG / 2.0)
            .expect_err("below analytical minimum")
    });
    assert_eq!(failure.code, ErrorCode::InvalidPatientWeight);
    let command = PageCommand::Error {
        identity: identity(PageRole::Form),
        request: Some(SequenceId::new(1).expect("request")),
        status: "rejected".into(),
        error_code: failure.code.to_wire(),
        message: failure.message,
    };
    let wire = tracing::info_span!("act")
        .in_scope(|| encode_page_command(&command, BYTE_BUDGET).expect("clinical error codec"));
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(serde_json::from_str::<Value>(&wire).expect("error object"),json!({"type":"error","identity":wire_identity("form"),
            "request_id":"1","status":"rejected","error_code":0x3001,"message":"Weight 0.125 kg outside demonstration bounds [0.25, 500]"}));
        assert_eq!(decode_page_command(&wire,BYTE_BUDGET).expect("error context round trip"),command);
    });
}

#[test]
fn shared_permission_codec_preserves_each_kind_and_native_initiation() {
    let _trace = capture();
    let _test = tracing::info_span!("native_shared_permission_command").entered();
    for (kind, label) in PageCapability::ALL.into_iter().zip([
        "geolocation",
        "camera",
        "microphone",
        "notifications",
    ]) {
        for initiated in [false, true] {
            let command = PageCommand::PermissionDenied {
                identity: identity(PageRole::PermissionProbe),
                kind,
                user_initiated: initiated,
            };
            let wire = tracing::info_span!("act", label, initiated).in_scope(|| {
                encode_page_command(&command, BYTE_BUDGET).expect("native observation codec")
            });
            tracing::info_span!("assert",label,initiated).in_scope(|| {
                assert_eq!(serde_json::from_str::<Value>(&wire).expect("permission object"),json!({"type":"permission_denied", "identity":wire_identity("permission_probe"),"permission":label,"user_initiated":initiated}));
                assert_eq!(decode_page_command(&wire,BYTE_BUDGET).expect("permission round trip"),command);
            });
        }
    }
}

#[test]
fn shared_submit_codec_rejects_unknown_fields_and_preserves_unicode_inputs() {
    let _trace = capture();
    let _test = tracing::info_span!("native_shared_submit_inputs").entered();
    let event = tracing::info_span!("arrange").in_scope(|| PageEvent::Submit {
        identity: identity(PageRole::Form),
        request: SequenceId::new(1).expect("request"),
        inputs: FormInputs::new("PT-1-患者", 72.5, 4.0, 0.5),
    });
    let wire = tracing::info_span!("act")
        .in_scope(|| encode_page_event(&event, BYTE_BUDGET).expect("submit codec"));
    tracing::info_span!("assert").in_scope(|| {
        let decoded = decode_page_event(&wire, BYTE_BUDGET).expect("submit decoded");
        assert_eq!(decoded, event);
        let PageEvent::Submit { inputs, .. } = decoded else {
            panic!("submit event");
        };
        assert_eq!(inputs.patient_id, "PT-1-患者");
        assert_eq!(inputs.weight_kg.to_bits(), 72.5_f64.to_bits());
        assert_eq!(inputs.concentration_mg_ml.to_bits(), 4.0_f64.to_bits());
        assert_eq!(inputs.target_dose_mcg_kg_min.to_bits(), 0.5_f64.to_bits());
        let malformed = wire.replacen("\"type\":", "\"extra\":true,\"type\":", 1);
        let error = decode_page_event(&malformed, BYTE_BUDGET).expect_err("unknown field");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("unknown field `extra`"));
    });
}

#[test]
fn shared_probe_completion_requires_all_four_typed_outcomes() {
    let _trace = capture();
    let _test = tracing::info_span!("native_shared_probe_completion").entered();
    // Value-semantic wire vectors, not browser or host-denial execution evidence.
    let observations = PageCapability::ALL.map(|kind| CapabilityObservation {
        kind,
        host_denial: None,
        browser: if kind == PageCapability::Notifications {
            BrowserPermissionResult::Denied
        } else {
            BrowserPermissionResult::Rejected {
                name: None,
                code: Some(1),
                message: format!("{} wire rejection context", kind.as_str()),
            }
        },
    });
    let event = PageEvent::ProbeCompleted {
        identity: identity(PageRole::PermissionProbe),
        observations: Box::new(observations),
        resources: ResourceCounts {
            dom_listeners: 2,
            bridge_listeners: 1,
            geolocation_dispatchers: 2,
            ..ResourceCounts::default()
        },
    };
    let wire = tracing::info_span!("act")
        .in_scope(|| encode_page_event(&event, BYTE_BUDGET).expect("four outcome codec"));
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(
            decode_page_event(&wire, BYTE_BUDGET).expect("four outcomes"),
            event
        );
        let mut value: Value = serde_json::from_str(&wire).expect("full object");
        assert_eq!(
            value["observations"]
                .as_array()
                .expect("outcome sequence")
                .len(),
            4
        );
        value["observations"]
            .as_array_mut()
            .expect("outcome sequence")
            .pop();
        assert_eq!(
            decode_page_event(&value.to_string(), BYTE_BUDGET)
                .expect_err("count alone is insufficient")
                .kind(),
            std::io::ErrorKind::InvalidData
        );
        assert_eq!(
            decode_page_event(r#"{"action":"permission_probe_complete"}"#, BYTE_BUDGET)
                .expect_err("retired shape rejected")
                .kind(),
            std::io::ErrorKind::InvalidData
        );
    });
}
