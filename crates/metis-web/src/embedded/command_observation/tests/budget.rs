//! Inclusive resource bounds and atomic transitions follow arithmetic models.

use super::{identity, trace};
use crate::embedded::{
    SequenceId,
    command_observation::{
        Budget, Event, MAX_BYTES, MAX_RECORDS, MAX_TERMINAL_BYTES, Record, encode_observation,
    },
};
use std::io::ErrorKind;

#[test]
fn one_terminal_record_remains_reserved_at_the_inclusive_count_limit() {
    let _trace = trace();
    let mut budget = tracing::info_span!("arrange").in_scope(Budget::default);
    // The accounting API consumes positive byte counts independently from JSON
    // validation. One is the analytical minimum positive count in that domain.
    for expected in 1..MAX_RECORDS {
        let admitted = tracing::info_span!("act", expected)
            .in_scope(|| budget.admit(1, false).expect("positive accounting input"));
        tracing::info_span!("assert", expected).in_scope(|| {
            assert!(admitted);
            assert_eq!(budget.records(), expected);
            assert_eq!(budget.bytes(), expected);
            assert!(!budget.is_terminal());
        });
    }
    let before = budget.clone();
    let rejected =
        tracing::info_span!("act").in_scope(|| budget.admit(1, false).expect("ordinary count cap"));
    tracing::info_span!("assert").in_scope(|| {
        assert!(!rejected);
        assert_eq!(budget, before);
    });
    let terminal = tracing::info_span!("act").in_scope(|| {
        budget
            .admit(MAX_TERMINAL_BYTES, true)
            .expect("reserved terminal")
    });
    tracing::info_span!("assert").in_scope(|| {
        assert!(terminal);
        assert_eq!(budget.records(), MAX_RECORDS);
        assert_eq!(budget.bytes(), MAX_RECORDS - 1 + MAX_TERMINAL_BYTES);
        assert!(budget.is_terminal());
    });
}

#[test]
fn byte_reservation_admits_the_exact_terminal_and_rejections_preserve_state() {
    let _trace = trace();
    let mut budget = tracing::info_span!("arrange").in_scope(Budget::default);
    let data_bytes = MAX_BYTES - MAX_TERMINAL_BYTES;
    let admitted = tracing::info_span!("act").in_scope(|| {
        budget
            .admit(data_bytes, false)
            .expect("exact data byte boundary")
    });
    tracing::info_span!("assert").in_scope(|| {
        assert!(admitted);
        assert_eq!(
            (budget.records(), budget.bytes(), budget.is_terminal()),
            (1, data_bytes, false)
        );
    });
    let before = budget.clone();
    for (bytes, terminal) in [(1, false), (MAX_TERMINAL_BYTES + 1, true)] {
        let actual = tracing::info_span!("act", bytes, terminal).in_scope(|| {
            budget
                .admit(bytes, terminal)
                .expect("resource cap is a rejection")
        });
        tracing::info_span!("assert").in_scope(|| {
            assert!(!actual);
            assert_eq!(budget, before);
        });
    }
    let terminal = tracing::info_span!("act").in_scope(|| {
        budget
            .admit(MAX_TERMINAL_BYTES, true)
            .expect("exact terminal byte limit")
    });
    tracing::info_span!("assert").in_scope(|| {
        assert!(terminal);
        assert_eq!(
            (budget.records(), budget.bytes(), budget.is_terminal()),
            (2, MAX_BYTES, true)
        );
    });
}

#[test]
fn empty_overflow_and_after_terminal_errors_preserve_exact_prior_counters() {
    let _trace = trace();
    let (mut budget, bytes) = tracing::info_span!("arrange").in_scope(|| {
        let value = Record::new(
            identity(1),
            SequenceId::new(1).expect("sequence"),
            None,
            Event::Selected,
        )
        .expect("selected observation");
        let bytes = encode_observation(&value, MAX_BYTES)
            .expect("actual encoded record")
            .len();
        (Budget::default(), bytes)
    });
    let empty =
        tracing::info_span!("act").in_scope(|| budget.admit(0, false).expect_err("empty encoding"));
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(empty.kind(), ErrorKind::InvalidInput);
        assert_eq!(
            (budget.records(), budget.bytes(), budget.is_terminal()),
            (0, 0, false)
        );
    });
    let accepted = tracing::info_span!("act")
        .in_scope(|| budget.admit(bytes, false).expect("actual selected bytes"));
    tracing::info_span!("assert").in_scope(|| assert!(accepted));
    let before = budget.clone();
    let overflow = tracing::info_span!("act").in_scope(|| {
        budget
            .admit(usize::MAX, false)
            .expect_err("checked sum overflow")
    });
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(overflow.kind(), ErrorKind::InvalidData);
        assert!(overflow.to_string().contains("arithmetic"));
        assert_eq!(budget, before);
    });
    let terminal = tracing::info_span!("act").in_scope(|| {
        budget
            .admit(MAX_TERMINAL_BYTES, true)
            .expect("reserved close")
    });
    tracing::info_span!("assert").in_scope(|| assert!(terminal));
    let before = budget.clone();
    let closed = tracing::info_span!("act").in_scope(|| {
        budget
            .admit(bytes, false)
            .expect_err("closed accounting owner")
    });
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(closed.kind(), ErrorKind::InvalidData);
        assert!(closed.to_string().contains("terminal"));
        assert_eq!(budget, before);
    });
}

#[test]
fn semantic_terminal_decode_is_separate_from_each_owners_actual_byte_admission() {
    let _trace = trace();
    let (expected, canonical, original) = tracing::info_span!("arrange").in_scope(|| {
        let sequence = SequenceId::new(2).expect("positive sequence");
        let expected = Record::new(identity(1), sequence, Some(sequence), Event::ClosingIntent)
            .expect("closing shape");
        let canonical = encode_observation(&expected, MAX_BYTES).expect("canonical closing");
        let padding = MAX_TERMINAL_BYTES + 1 - canonical.len();
        let original = format!("{}{canonical}", " ".repeat(padding));
        (expected, canonical, original)
    });
    let mut producer = Budget::default();
    let mut consumer = Budget::default();
    let (decoded, sent, retained) = tracing::info_span!("act").in_scope(|| {
        (
            super::super::decode_observation(&original, MAX_BYTES).expect("semantic original JSON"),
            producer
                .admit(canonical.len(), true)
                .expect("canonical producer bytes"),
            consumer
                .admit(original.len(), true)
                .expect("raw consumer byte cap"),
        )
    });
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(decoded, expected);
        assert!(sent && retained);
        assert_eq!(original.len(), MAX_TERMINAL_BYTES + 1);
        assert_eq!(
            (producer.records(), producer.bytes(), producer.is_terminal()),
            (1, canonical.len(), true)
        );
        assert_eq!(
            (consumer.records(), consumer.bytes(), consumer.is_terminal()),
            (1, original.len(), true)
        );
    });
}
