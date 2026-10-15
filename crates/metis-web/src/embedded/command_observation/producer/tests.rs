//! Value-semantic laws for the real portable producer, without a bridge substitute.

use super::*;
use crate::embedded::{
    PageRole, PageStage,
    command_observation::{
        Admission, Decision, KeyboardSnapshot, MAX_RECORDS, Outcome, decode_observation,
    },
};
use metis_core::embedded_assets::Digest;
use metis_frontend::ApplicationCommand;

fn trace() -> tracing::subscriber::DefaultGuard {
    tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_test_writer()
            .without_time()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .finish(),
    )
}

fn identity() -> PageIdentity {
    PageIdentity::new(
        PageRole::Form,
        SequenceId::new(7).expect("positive mount"),
        Digest::hash(b"producer law: exact prepared module bytes"),
    )
}

fn metadata(command: ApplicationCommand, key: &str, code: &str) -> Event {
    Event::MetadataDecision {
        keyboard: Some(KeyboardSnapshot {
            key: key.to_owned(),
            code: code.to_owned(),
            ctrl: false,
            alt: true,
            shift: true,
            meta: false,
            repeat: false,
            trusted: true,
        }),
        admission: Admission::Ready,
        decision: Decision::Command(command),
    }
}

fn proposed_frames() -> Vec<(SequenceId, Event)> {
    (1..=MAX_RECORDS)
        .flat_map(|index| {
            let invocation =
                SequenceId::new(u64::try_from(index).expect("valid producer-law value"))
                    .expect("valid producer-law value");
            let mut events = Vec::new();
            if index > 1 {
                events.push(Event::Entered {
                    stage: PageStage::Ready,
                });
            }
            let (command, key, code) = if index % 2 == 1 {
                (ApplicationCommand::ThemeDark, "D", "KeyD")
            } else {
                (ApplicationCommand::ThemeSystem, "S", "KeyS")
            };
            events.push(metadata(command, key, code));
            events.push(Event::Returned {
                outcome: Outcome::Ok,
            });
            events.into_iter().map(move |event| (invocation, event))
        })
        .collect()
}

fn canonical(sequence: usize, invocation: Option<SequenceId>, event: Event) -> String {
    let record = Record::new(
        identity(),
        SequenceId::new(u64::try_from(sequence).expect("valid producer-law value"))
            .expect("valid producer-law value"),
        invocation,
        event,
    )
    .expect("reference record has a valid identity and event shape");
    encode_observation(&record, MAX_BYTES).expect("reference record fits one message")
}

struct Prefix {
    frames: Vec<String>,
    rejected: String,
    rejected_index: usize,
    rejected_invocation: SequenceId,
    prior_bytes: usize,
}

struct ObservedAdmission {
    encoded: Option<String>,
    records: usize,
    bytes: usize,
    invocation: Option<SequenceId>,
}

fn maximal_prefix(proposals: &[(SequenceId, Event)]) -> Prefix {
    // This reference uses the specification's scalar inequalities, not Budget
    // or Producer admission. Every proposed row is encoded before comparison.
    let mut frames = vec![canonical(1, None, Event::Selected)];
    let mut bytes = frames[0].len();
    for (index, (invocation, event)) in proposals.iter().enumerate() {
        let encoded = canonical(frames.len() + 1, Some(*invocation), event.clone());
        let count_fits = frames.len() < MAX_RECORDS - 1;
        let bytes_fit = encoded.len() <= MAX_BYTES - MAX_TERMINAL_BYTES - bytes;
        if count_fits && bytes_fit {
            bytes += encoded.len();
            frames.push(encoded);
        } else {
            frames.push(canonical(
                frames.len() + 1,
                None,
                Event::Quota {
                    prior_records: u32::try_from(frames.len()).expect("valid producer-law value"),
                    prior_bytes: u32::try_from(bytes).expect("valid producer-law value"),
                    omitted_at_notice: 1,
                    failed: false,
                },
            ));
            return Prefix {
                frames,
                rejected: encoded,
                rejected_index: index,
                rejected_invocation: *invocation,
                prior_bytes: bytes,
            };
        }
    }
    panic!("the real proposed stream must exceed its finite byte budget");
}

fn observe_admissions(
    owner: &mut Producer,
    proposals: &[(SequenceId, Event)],
) -> Vec<ObservedAdmission> {
    let mut invocation = owner.arm();
    let mut previous = proposals[0].0;
    proposals
        .iter()
        .map(|(expected, event)| {
            if *expected != previous {
                invocation = owner.next_invocation();
                previous = *expected;
            }
            let encoded = owner.prepare(invocation, event.clone());
            ObservedAdmission {
                encoded,
                records: owner.budget.records(),
                bytes: owner.budget.bytes(),
                invocation,
            }
        })
        .collect()
}

fn assert_admissions(
    proposals: &[(SequenceId, Event)],
    reference: &Prefix,
    admissions: &[ObservedAdmission],
) {
    assert_eq!(admissions.len(), proposals.len());
    for (index, actual) in admissions.iter().enumerate() {
        assert_eq!(
            actual.encoded.as_ref(),
            reference.frames.get(index + 1),
            "proposal {index}"
        );
        let expected_records = (index + 2).min(reference.frames.len());
        assert_eq!(actual.records, expected_records, "proposal {index} count");
        assert_eq!(
            actual.bytes,
            reference.frames[..expected_records]
                .iter()
                .map(String::len)
                .sum::<usize>(),
            "proposal {index} bytes"
        );
        let expected_invocation =
            (proposals[index].0 <= reference.rejected_invocation).then_some(proposals[index].0);
        assert_eq!(
            actual.invocation, expected_invocation,
            "proposal {index} invocation"
        );
    }
}

#[test]
fn arming_retains_exact_decision_and_later_entry_without_inventing_first_entry() {
    let _trace = trace();
    let _test = tracing::info_span!("document_producer_arming").entered();
    let mut owner = tracing::info_span!("arrange").in_scope(|| Producer::new(identity()));
    let frames = tracing::info_span!("act").in_scope(|| {
        let selected = owner.selected().expect("selected record");
        assert!(
            owner.next_invocation().is_none(),
            "unarmed has no invocation"
        );
        let first = owner.arm().expect("actual Dark resolution arms");
        let dark = owner
            .prepare(
                Some(first),
                metadata(ApplicationCommand::ThemeDark, "D", "KeyD"),
            )
            .expect("same Dark metadata");
        let returned = owner
            .prepare(
                Some(first),
                Event::Returned {
                    outcome: Outcome::Ok,
                },
            )
            .expect("actual handler result");
        let next = owner.next_invocation().expect("next real callback");
        let entered = owner
            .prepare(
                Some(next),
                Event::Entered {
                    stage: PageStage::Ready,
                },
            )
            .expect("entry before the next metadata read");
        let system = owner
            .prepare(
                Some(next),
                metadata(ApplicationCommand::ThemeSystem, "S", "KeyS"),
            )
            .expect("same System metadata");
        (vec![selected, dark, returned, entered, system], first, next)
    });
    tracing::info_span!("assert").in_scope(|| {
        let (frames, first, next) = frames;
        assert_eq!((first.get(), next.get()), (1, 2));
        let rows = frames
            .iter()
            .map(|raw| decode_observation(raw, MAX_BYTES).expect("valid producer-law value"))
            .collect::<Vec<_>>();
        assert!(matches!(rows[0].event(), Event::Selected));
        assert!(matches!(rows[1].event(), Event::MetadataDecision { .. }));
        assert!(matches!(
            rows[3].event(),
            Event::Entered {
                stage: PageStage::Ready
            }
        ));
        let Event::MetadataDecision {
            keyboard: Some(keyboard),
            decision,
            ..
        } = rows[4].event()
        else {
            panic!("System must retain its actual keyboard record");
        };
        assert_eq!(
            (keyboard.key.as_str(), keyboard.code.as_str()),
            ("S", "KeyS")
        );
        assert!(keyboard.alt && keyboard.shift && keyboard.trusted);
        assert!(!keyboard.ctrl && !keyboard.meta && !keyboard.repeat);
        assert!(matches!(
            decision,
            Decision::Command(ApplicationCommand::ThemeSystem)
        ));
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(row.identity(), identity());
            assert_eq!(
                row.sequence().get(),
                u64::try_from(index + 1).expect("valid producer-law value")
            );
        }
        assert_eq!(owner.budget.records(), frames.len());
        assert_eq!(
            owner.budget.bytes(),
            frames.iter().map(String::len).sum::<usize>()
        );
        assert!(owner.armed() && owner.failure.is_none());
    });
}

#[test]
fn exhaustion_emits_one_reserved_terminal_with_exact_prior_accounting() {
    let _trace = trace();
    let _test = tracing::info_span!("document_producer_quota").entered();
    let (mut owner, proposals, reference) = tracing::info_span!("arrange").in_scope(|| {
        let proposals = proposed_frames();
        let reference = maximal_prefix(&proposals);
        (Producer::new(identity()), proposals, reference)
    });
    let (selected, admissions) = tracing::info_span!("act").in_scope(|| {
        let selected = owner.selected().expect("actual selected record");
        let admissions = observe_admissions(&mut owner, &proposals);
        (selected, admissions)
    });
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(selected, reference.frames[0]);
        assert_admissions(&proposals, &reference, &admissions);
        let frames = &reference.frames;
        let prior_records = frames.len() - 1;
        assert_eq!(reference.rejected_index + 1, prior_records);
        assert!(prior_records < MAX_RECORDS - 1, "bytes bind before count");
        assert!(reference.prior_bytes <= MAX_BYTES - MAX_TERMINAL_BYTES);
        assert!(reference.prior_bytes + reference.rejected.len() > MAX_BYTES - MAX_TERMINAL_BYTES);
        let last = decode_observation(
            frames.last().expect("valid producer-law value"),
            MAX_TERMINAL_BYTES,
        )
        .expect("valid producer-law value");
        let Event::Quota {
            prior_records,
            prior_bytes,
            omitted_at_notice,
            failed,
        } = last.event()
        else {
            panic!("exhaustion must report a real terminal");
        };
        assert_eq!(
            usize::try_from(*prior_records).expect("valid producer-law value"),
            frames.len() - 1
        );
        assert_eq!(
            usize::try_from(*prior_bytes).expect("valid producer-law value"),
            frames[..frames.len() - 1]
                .iter()
                .map(String::len)
                .sum::<usize>()
        );
        assert_eq!(*omitted_at_notice, 1);
        assert!(!failed);
        assert!(frames.len() <= MAX_RECORDS);
        assert_eq!(owner.budget.records(), frames.len());
        assert_eq!(
            owner.budget.bytes(),
            frames.iter().map(String::len).sum::<usize>()
        );
        assert!(owner.budget.bytes() <= MAX_BYTES);
        assert!(owner.budget.is_terminal() && !owner.active());
        assert!(owner.prepare(None, Event::Selected).is_none());
        assert!(owner.next_invocation().is_none());
        assert!(owner.failure.is_none() && owner.terminal_failure.is_none());
    });
}

#[test]
fn canonical_frame_minimum_makes_producer_bytes_bind_before_record_count() {
    let _trace = trace();
    let _test = tracing::info_span!("document_producer_minimum_frame").entered();
    let minimum_identity = tracing::info_span!("arrange").in_scope(|| {
        PageIdentity::new(
            PageRole::Form,
            SequenceId::new(1).expect("valid producer-law value"),
            Digest::hash(b"minimum-width canonical identity for a real module digest"),
        )
    });
    let shortest = tracing::info_span!("act").in_scope(|| {
        let record = Record::new(
            minimum_identity,
            SequenceId::new(1).expect("valid producer-law value"),
            None,
            Event::Selected,
        )
        .expect("valid producer-law value");
        encode_observation(&record, MAX_BYTES).expect("valid producer-law value")
    });
    tracing::info_span!("assert").in_scope(|| {
        // Form and one-digit canonical integers minimize the fixed envelope.
        // Selected has the shortest ordinary event body. A present invocation
        // can save one byte versus null, but Entered/Returned add at least 15
        // body bytes; MetadataDecision is larger still. Terminal events do not
        // consume the ordinary allowance. All digests occupy 64 ASCII bytes.
        assert_eq!(shortest.len(), 256);
        assert_eq!(
            decode_observation(&shortest, MAX_BYTES)
                .expect("valid producer-law value")
                .identity(),
            minimum_identity
        );
        let ordinary_bytes = MAX_BYTES - MAX_TERMINAL_BYTES;
        assert_eq!((MAX_RECORDS - 1) * shortest.len(), 32_512);
        assert!((MAX_RECORDS - 1) * shortest.len() > ordinary_bytes);
        // The scalar-length count boundary remains a separate Budget law;
        // shorter fabricated Producer rows cannot establish that coverage.
        assert!(shortest.len() > ordinary_bytes / (MAX_RECORDS - 1));
    });
}

#[test]
fn closing_intent_ends_production_without_claiming_handler_or_lifecycle_success() {
    let _trace = trace();
    let _test = tracing::info_span!("document_producer_closing_intent").entered();
    let mut owner = tracing::info_span!("arrange").in_scope(|| Producer::new(identity()));
    let (selected, terminal) = tracing::info_span!("act").in_scope(|| {
        let selected = owner.selected().expect("valid producer-law value");
        let invocation = owner.arm().expect("valid producer-law value");
        (
            selected,
            owner
                .prepare(Some(invocation), Event::ClosingIntent)
                .expect("valid producer-law value"),
        )
    });
    tracing::info_span!("assert").in_scope(|| {
        let record =
            decode_observation(&terminal, MAX_TERMINAL_BYTES).expect("valid producer-law value");
        assert!(matches!(record.event(), Event::ClosingIntent));
        assert_eq!(
            record.invocation().expect("valid producer-law value").get(),
            1
        );
        assert_eq!(record.sequence().get(), 2);
        assert_eq!(owner.budget.bytes(), selected.len() + terminal.len());
        assert!(owner.budget.is_terminal());
        assert!(
            owner
                .prepare(
                    record.invocation(),
                    Event::Returned {
                        outcome: Outcome::Ok
                    }
                )
                .is_none()
        );
    });
}

#[test]
fn encoding_failure_retains_typed_source_and_reports_a_failed_reserved_notice() {
    let _trace = trace();
    let _test = tracing::info_span!("document_producer_failure").entered();
    let mut owner = tracing::info_span!("arrange").in_scope(|| Producer::new(identity()));
    let terminal = tracing::info_span!("act").in_scope(|| {
        owner.selected().expect("valid producer-law value");
        let invocation = owner.arm().expect("valid producer-law value");
        owner
            .prepare(
                Some(invocation),
                metadata(
                    ApplicationCommand::ThemeDark,
                    &"x".repeat(MAX_BYTES),
                    "KeyD",
                ),
            )
            .expect("valid producer-law value")
    });
    tracing::info_span!("assert").in_scope(|| {
        let record =
            decode_observation(&terminal, MAX_TERMINAL_BYTES).expect("valid producer-law value");
        assert!(matches!(
            record.event(),
            Event::Quota {
                failed: true,
                omitted_at_notice: 1,
                ..
            }
        ));
        let failure = owner
            .failure
            .as_ref()
            .expect("actual encoding failure stays owned");
        assert_eq!(failure.kind(), io::ErrorKind::InvalidInput);
        assert!(failure.to_string().contains("byte budget"));
        assert!(owner.terminal_failure.is_none());
        assert!(!owner.active());
        let first = owner
            .failure
            .as_ref()
            .expect("valid producer-law value")
            .to_string();
        owner.disable(io::Error::other("a later publication failure"));
        assert_eq!(
            owner
                .failure
                .as_ref()
                .expect("valid producer-law value")
                .to_string(),
            first
        );
        assert!(owner.prepare(None, Event::Selected).is_none());
    });
}
