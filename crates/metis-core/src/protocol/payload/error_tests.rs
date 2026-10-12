//! Error bodies obey the same exact-length contract as successful payloads.
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
    let arrange = tracing::info_span!("arrange").entered();
    let error = ErrorResponsePayload {
        error_code: ErrorCode::MalformedPayload as u16,
        message: "Δ invalid 🦀 input".to_owned(),
    };
    drop(arrange);
    let act = tracing::info_span!("act").entered();
    let encoded = error.encode().expect("error body");
    drop(act);
    let _assert = tracing::info_span!("assert").entered();
    assert_eq!(
        ErrorResponsePayload::decode(&encoded).expect("decoded error"),
        error
    );
    assert_eq!(&encoded[..2], &0x1007_u16.to_be_bytes());
    assert_eq!(&encoded[4..], error.message.as_bytes());
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
