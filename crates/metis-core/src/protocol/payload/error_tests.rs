//! Error bodies preserve wire codes, Unicode and exact message lengths.
use super::{ErrorCode, ErrorResponsePayload};

#[test]
fn error_body_preserves_unicode_code_and_rejects_every_trailing_suffix() {
    let _trace = tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_test_writer()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .finish(),
    );
    let _test = tracing::info_span!("error_payload").entered();
    // Published wire discriminants plus an unknown future code. The inverse
    // mapping verifies the enum contract without introducing an enum cast.
    for (code, message, classification) in [
        (
            0x1007,
            "Δ invalid 🦀 input",
            Some(ErrorCode::MalformedPayload),
        ),
        (0x4003, "deadline", Some(ErrorCode::Timeout)),
        (0xffff, "", None),
    ] {
        let arrange = tracing::info_span!("arrange", code).entered();
        let error = ErrorResponsePayload {
            error_code: code,
            message: message.to_owned(),
        };
        drop(arrange);
        let act = tracing::info_span!("act").entered();
        let encoded = error.encode().expect("error body");
        let decoded = ErrorResponsePayload::decode(&encoded).expect("decoded error");
        drop(act);
        let _assert = tracing::info_span!("assert").entered();
        assert_eq!(ErrorCode::from_wire(code), classification);
        assert_eq!(decoded, error);
        assert_eq!(&encoded[..2], &code.to_be_bytes());
        assert_eq!(&encoded[4..], message.as_bytes());
        for suffix in [vec![1], vec![0xff, 0xfe], b"trailing".to_vec()] {
            let mut bytes = encoded.clone();
            bytes.extend(suffix);
            assert_eq!(
                ErrorResponsePayload::decode(&bytes)
                    .expect_err("trailing bytes")
                    .code,
                ErrorCode::MalformedPayload
            );
        }
        for length in 0..encoded.len() {
            assert_eq!(
                ErrorResponsePayload::decode(&encoded[..length])
                    .expect_err("truncation")
                    .code,
                ErrorCode::MalformedPayload
            );
        }
    }
}
