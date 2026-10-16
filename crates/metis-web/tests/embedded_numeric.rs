//! Binary64 JSON representation is exact; backend range admission is separate.

use metis_core::{embedded_assets::Digest, protocol::ClinicalCalcResponsePayload};
use metis_web::embedded::{
    PageCommand, PageIdentity, PageRole, SequenceId, decode_page_command, encode_page_command,
};

#[test]
fn every_finite_exponent_sign_and_mantissa_boundary_preserves_its_bits() {
    let _trace = tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .with_test_writer()
            .finish(),
    );
    let identity = tracing::info_span!("arrange").in_scope(|| {
        PageIdentity::new(
            PageRole::Form,
            SequenceId::new(1).expect("positive mount"),
            Digest::hash(b"binary64 codec oracle"),
        )
    });
    // IEEE 754 binary64 has one sign bit, 11 exponent bits and 52 fraction
    // bits. Exponent 2047 is nonfinite. Each finite exponent is checked at
    // both interval endpoints and the adjacent representable fraction.
    for sign in [0, 1_u64 << 63] {
        for exponent in 0_u64..2047 {
            for fraction in [0, 1, (1_u64 << 52) - 1] {
                let bits = sign | (exponent << 52) | fraction;
                let value = f64::from_bits(bits);
                let expected = PageCommand::Result {
                    identity,
                    request: SequenceId::new(1).expect("request"),
                    response: ClinicalCalcResponsePayload {
                        audit_sequence_id: 1,
                        rate_ml_hr: value,
                        drug_rate_mg_hr: value,
                        is_pediatric: false,
                        result_signature: Digest::hash(&bits.to_be_bytes()).into_bytes(),
                    },
                };
                let actual = tracing::info_span!("act").in_scope(|| {
                    let json = encode_page_command(&expected, 64 * 1024).expect("finite encoding");
                    decode_page_command(&json, 64 * 1024).expect("finite decoding")
                });
                tracing::info_span!("assert").in_scope(|| {
                    let PageCommand::Result { response, .. } = actual else {
                        panic!("result codec changes the command")
                    };
                    assert_eq!(
                        response.rate_ml_hr.to_bits(),
                        bits,
                        "rate bits={bits:#018x}"
                    );
                    assert_eq!(
                        response.drug_rate_mg_hr.to_bits(),
                        bits,
                        "drug bits={bits:#018x}"
                    );
                });
            }
        }
    }
}

#[test]
fn decimal_strings_reject_numeric_json_noncanonical_and_nonfinite_values() {
    let _trace = tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .with_test_writer()
            .finish(),
    );
    let command = tracing::info_span!("arrange").in_scope(|| PageCommand::Result {
        identity: PageIdentity::new(
            PageRole::Form,
            SequenceId::new(1).expect("mount"),
            Digest::hash(b"closed decimal number oracle"),
        ),
        request: SequenceId::new(1).expect("request"),
        response: ClinicalCalcResponsePayload {
            audit_sequence_id: 1,
            rate_ml_hr: -0.0,
            drug_rate_mg_hr: 1.875,
            is_pediatric: false,
            result_signature: Digest::hash(b"result MAC representation").into_bytes(),
        },
    });
    let json = encode_page_command(&command, 64 * 1024).expect("finite encoding");
    assert!(json.contains("\"rate_ml_hr\":\"-0.0\""));
    for field in ["rate_ml_hr", "drug_rate_mg_hr"] {
        let mut value: serde_json::Value = serde_json::from_str(&json).expect("encoded object");
        for replacement in [
            serde_json::json!(0.0),
            serde_json::json!("0"),
            serde_json::json!("01.0"),
            serde_json::json!("+1.0"),
            serde_json::json!(" 1.0"),
            serde_json::json!("1.0 "),
            serde_json::json!("1.8750"),
            serde_json::json!("1.875e0"),
            serde_json::json!("NaN"),
            serde_json::json!("Infinity"),
            serde_json::json!("1e309"),
            serde_json::json!("1e-999"),
            serde_json::json!("1234567890123456789012345"),
        ] {
            value[field] = replacement;
            let malformed = serde_json::to_string(&value).expect("malformed wire serialization");
            let error = tracing::info_span!("act", field).in_scope(|| {
                decode_page_command(&malformed, 64 * 1024).expect_err("closed decimal domain")
            });
            tracing::info_span!("assert", field).in_scope(|| {
                assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
                assert!(!error.to_string().is_empty());
            });
        }
    }
}
