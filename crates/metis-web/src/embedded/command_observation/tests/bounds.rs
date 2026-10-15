//! Real encodings exercise inclusive UTF-8 and finite terminal bounds.

use super::{identity, trace};
use crate::embedded::{
    PageIdentity, PageRole, SequenceId,
    command_observation::{
        Admission, Decision, Event, KeyboardSnapshot, MAX_BYTES, MAX_TERMINAL_BYTES, Record,
        decode_observation, encode_observation,
    },
};
use metis_core::embedded_assets::Digest;
use std::io::ErrorKind;

#[test]
fn encoding_and_original_decoding_use_inclusive_bytes_without_metadata_truncation() {
    let _trace = trace();
    let expected = tracing::info_span!("arrange").in_scope(|| {
        Record::new(
            identity(1),
            SequenceId::new(3).expect("sequence"),
            Some(SequenceId::new(2).expect("invocation")),
            Event::MetadataDecision {
                keyboard: Some(KeyboardSnapshot {
                    key: "患者🦀\n\"".repeat(129),
                    code: "KeyS".repeat(17),
                    ctrl: false,
                    alt: true,
                    shift: true,
                    meta: false,
                    repeat: false,
                    trusted: true,
                }),
                admission: Admission::Ready,
                decision: Decision::Ignored,
            },
        )
        .expect("coherent complete metadata")
    });
    let json = tracing::info_span!("act")
        .in_scope(|| encode_observation(&expected, MAX_BYTES).expect("bounded Unicode record"));
    let (encoded, decoded, encode_error, decode_error) =
        tracing::info_span!("act").in_scope(|| {
            (
                encode_observation(&expected, json.len()).expect("inclusive encode"),
                decode_observation(&json, json.len()).expect("inclusive original decode"),
                encode_observation(&expected, json.len() - 1).expect_err("one byte beyond encode"),
                decode_observation(&json, json.len() - 1).expect_err("one byte beyond decode"),
            )
        });
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(encoded, json);
        assert_eq!(decoded, expected);
        assert_eq!(encode_error.kind(), ErrorKind::InvalidInput);
        assert_eq!(decode_error.kind(), ErrorKind::InvalidData);
        let Event::MetadataDecision {
            keyboard: Some(actual),
            ..
        } = decoded.event()
        else {
            panic!("actual metadata disappears");
        };
        assert_eq!(actual.key, "患者🦀\n\"".repeat(129));
        assert_eq!(actual.code, "KeyS".repeat(17));
        assert!(actual.key.len() > 64 && actual.code.len() > 64);
    });
}

#[test]
fn both_terminal_schemas_fit_the_const_bound_at_every_maximal_field_width() {
    let _trace = trace();
    let mut maximum = 0;
    for role in [PageRole::Form, PageRole::PermissionProbe] {
        for failed in [false, true] {
            let (identity, sequence) = tracing::info_span!("arrange").in_scope(|| {
                let sequence = SequenceId::new(u64::MAX).expect("maximum sequence");
                (
                    PageIdentity::new(role, sequence, Digest::hash(b"max-width terminal identity")),
                    sequence,
                )
            });
            for (invocation, event) in [
                (
                    None,
                    Event::Quota {
                        prior_records: u32::MAX,
                        prior_bytes: u32::MAX,
                        omitted_at_notice: u64::MAX,
                        failed,
                    },
                ),
                (Some(sequence), Event::ClosingIntent),
            ] {
                let expected = tracing::info_span!("arrange").in_scope(|| {
                    Record::new(identity, sequence, invocation, event).expect("terminal shape")
                });
                let (json, actual) = tracing::info_span!("act").in_scope(|| {
                    let json = encode_observation(&expected, MAX_TERMINAL_BYTES)
                        .expect("derived terminal cap");
                    let actual =
                        decode_observation(&json, json.len()).expect("exact terminal bytes");
                    (json, actual)
                });
                tracing::info_span!("assert").in_scope(|| {
                    assert!(expected.is_terminal());
                    assert_eq!(actual, expected);
                    assert!(json.len() <= MAX_TERMINAL_BYTES);
                    maximum = maximum.max(json.len());
                });
            }
        }
    }
    tracing::info_span!("assert").in_scope(|| assert_eq!(maximum, MAX_TERMINAL_BYTES));
}
