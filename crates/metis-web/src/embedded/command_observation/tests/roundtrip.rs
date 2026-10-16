//! Exact typed values survive the real codec without reinterpreting policy.

use super::super::{
    Admission, Decision, Event, KeyboardSnapshot, MAX_BYTES, Outcome, Record, decode_observation,
    encode_observation, is_observation,
};
use super::{identity, trace};
use crate::embedded::{PageIdentity, PageRole, PageStage, SequenceId};
use metis_frontend::ApplicationCommand;

fn metadata_events() -> Vec<Event> {
    let keyboard = KeyboardSnapshot {
        key: "患者-🦀-S".into(),
        code: "KeyS".into(),
        ctrl: true,
        alt: false,
        shift: true,
        meta: false,
        repeat: true,
        trusted: false,
    };
    let mut decisions = vec![
        Decision::Repeat,
        Decision::EscapeMenu,
        Decision::EscapeClose,
        Decision::Ignored,
    ];
    decisions.extend(ApplicationCommand::ALL.map(Decision::Command));
    let mut events: Vec<_> = decisions
        .into_iter()
        .map(|decision| Event::MetadataDecision {
            keyboard: Some(keyboard.clone()),
            admission: Admission::Ready,
            decision,
        })
        .collect();
    events.extend(
        [
            Admission::NotReady,
            Admission::NotKeyboard,
            Admission::MetadataError,
        ]
        .map(|admission| Event::MetadataDecision {
            keyboard: None,
            admission,
            decision: Decision::NotEvaluated,
        }),
    );
    events.push(Event::MetadataDecision {
        keyboard: Some(KeyboardSnapshot {
            key: "Σ-\"\\\n".into(),
            code: "Unidentified".into(),
            ctrl: false,
            alt: true,
            shift: false,
            meta: true,
            repeat: false,
            trusted: true,
        }),
        admission: Admission::Ready,
        decision: Decision::Ignored,
    });
    events
}

fn all_events() -> Vec<Event> {
    let mut events = vec![
        Event::Selected,
        Event::ClosingIntent,
        Event::Quota {
            prior_records: u32::MAX,
            prior_bytes: u32::MAX,
            omitted_at_notice: u64::MAX,
            failed: true,
        },
    ];
    events.extend(
        [
            PageStage::Loading,
            PageStage::Ready,
            PageStage::Stopping,
            PageStage::Stopped,
            PageStage::Closed,
        ]
        .map(|stage| Event::Entered { stage }),
    );
    events.extend(
        [
            Outcome::Ok,
            Outcome::ReadyRejected,
            Outcome::MetadataError,
            Outcome::ActionError,
        ]
        .map(|outcome| Event::Returned { outcome }),
    );
    events.extend(metadata_events());
    events
}

#[test]
fn every_event_stage_command_and_identity_keeps_its_exact_typed_values() {
    let _trace = trace();
    let _test = tracing::info_span!("command_observation_typed_roundtrip").entered();
    let events = tracing::info_span!("arrange").in_scope(all_events);
    // Distinct values expose a codec that exchanges identity, sequence or invocation.
    let domains = [
        (1, 3, 2),
        ((1 << 53) + 1, (1 << 53) + 3, (1 << 53) + 2),
        (u64::MAX, u64::MAX - 1, u64::MAX - 2),
    ];
    for role in [PageRole::Form, PageRole::PermissionProbe] {
        for (mount, sequence, invocation) in domains {
            for event in &events {
                let expected = tracing::info_span!("arrange").in_scope(|| {
                    let owner = PageIdentity::new(
                        role,
                        SequenceId::new(mount).expect("positive mount"),
                        identity(mount).module_digest(),
                    );
                    let invocation = (!matches!(event, Event::Selected | Event::Quota { .. }))
                        .then(|| SequenceId::new(invocation).expect("positive invocation"));
                    Record::new(
                        owner,
                        SequenceId::new(sequence).expect("positive sequence"),
                        invocation,
                        event.clone(),
                    )
                    .expect("coherent typed observation")
                });
                let (encoded, actual) = tracing::info_span!("act").in_scope(|| {
                    let encoded = encode_observation(&expected, MAX_BYTES).expect("bounded record");
                    let actual = decode_observation(&encoded, MAX_BYTES).expect("exact record");
                    (encoded, actual)
                });
                tracing::info_span!("assert").in_scope(|| {
                    assert_eq!(actual, expected);
                    assert_eq!(actual.identity().role(), role);
                    assert_eq!(actual.identity().mount().get(), mount);
                    assert_eq!(actual.sequence().get(), sequence);
                    assert_eq!(actual.invocation(), expected.invocation());
                    assert_eq!(actual.event(), event);
                    assert_eq!(
                        actual.is_terminal(),
                        matches!(event, Event::ClosingIntent | Event::Quota { .. })
                    );
                    assert!(is_observation(&encoded));
                    let wire: serde_json::Value =
                        serde_json::from_str(&encoded).expect("actual wire");
                    assert_eq!(wire["sequence"], sequence.to_string());
                    assert_eq!(wire["identity"]["mount_id"], mount.to_string());
                    assert_eq!(wire["identity"]["role"], role.as_str());
                });
            }
        }
    }
}

#[test]
fn semantic_json_accepts_whitespace_reordering_and_equivalent_unicode_escapes() {
    let _trace = trace();
    let _test = tracing::info_span!("command_observation_semantic_json").entered();
    let (expected, canonical, alternatives) = tracing::info_span!("arrange").in_scope(|| {
        let event = metadata_events().into_iter().next().expect("repeat observation");
        let expected = Record::new(identity(7), SequenceId::new(13).expect("sequence"),
                                   Some(SequenceId::new(11).expect("invocation")), event)
            .expect("actual metadata record");
        let canonical = encode_observation(&expected, MAX_BYTES).expect("canonical encoding");
        let wire: serde_json::Value = serde_json::from_str(&canonical).expect("wire object");
        let reordered = format!(
            r#" {{ "event":{}, "invocation":{}, "sequence":{}, "identity":{}, "observation_schema":1, "kind":"metis_command_\u006fbservation" }} "#,
            wire["event"], wire["invocation"], wire["sequence"], wire["identity"],
        );
        let escaped = reordered.replace("患者", r"\u60a3\u8005").replace("🦀", r"\ud83e\udd80");
        let pretty = serde_json::to_string_pretty(&wire).expect("semantic pretty encoding");
        (expected, canonical, [reordered, escaped, pretty])
    });
    for original in alternatives {
        let actual = tracing::info_span!("act").in_scope(|| {
            decode_observation(&original, MAX_BYTES).expect("equivalent semantic observation")
        });
        tracing::info_span!("assert").in_scope(|| {
            assert_ne!(original, canonical);
            assert_eq!(actual, expected);
            assert!(is_observation(&original));
            let Event::MetadataDecision {
                keyboard: Some(keyboard),
                ..
            } = actual.event()
            else {
                panic!("decoded metadata snapshot is absent");
            };
            assert_eq!(keyboard.key, "患者-🦀-S");
            assert_eq!(keyboard.code, "KeyS");
            assert!(keyboard.ctrl && keyboard.shift && keyboard.repeat);
            assert!(!keyboard.alt && !keyboard.meta && !keyboard.trusted);
        });
    }
}

#[test]
fn quota_codec_preserves_full_counter_domains_without_budget_coupling() {
    let _trace = trace();
    let _test = tracing::info_span!("command_observation_quota_domains").entered();
    let omissions = tracing::info_span!("arrange")
        .in_scope(|| [0, 1, (1 << 53) - 1, 1 << 53, (1 << 53) + 1, u64::MAX]);
    for omitted_at_notice in omissions {
        for failed in [false, true] {
            let expected = Record::new(
                identity(u64::MAX),
                SequenceId::new(u64::MAX).expect("sequence"),
                None,
                Event::Quota {
                    prior_records: u32::MAX,
                    prior_bytes: u32::MAX,
                    omitted_at_notice,
                    failed,
                },
            )
            .expect("typed full-domain counters");
            let (encoded, actual) = tracing::info_span!("act").in_scope(|| {
                let encoded = encode_observation(&expected, MAX_BYTES).expect("quota encoding");
                let actual = decode_observation(&encoded, MAX_BYTES).expect("quota decoding");
                (encoded, actual)
            });
            tracing::info_span!("assert").in_scope(|| {
                assert_eq!(actual, expected);
                assert_eq!(actual.invocation(), None);
                assert!(actual.is_terminal());
                let wire: serde_json::Value = serde_json::from_str(&encoded).expect("wire quota");
                assert_eq!(wire["event"]["prior_records"], u32::MAX);
                assert_eq!(wire["event"]["prior_bytes"], u32::MAX);
                assert_eq!(
                    wire["event"]["omitted_at_notice"],
                    omitted_at_notice.to_string()
                );
                assert_eq!(wire["event"]["failed"], failed);
            });
        }
    }
}

#[test]
fn semantic_kind_detection_does_not_match_nested_or_unrelated_text() {
    let _trace = trace();
    let _test = tracing::info_span!("command_observation_kind_detection").entered();
    let originals = tracing::info_span!("arrange").in_scope(|| {
        [
            r#"{"kind":"page_event","message":"metis_command_observation"}"#,
            r#"{"wrapper":{"kind":"metis_command_observation"}}"#,
            r#""metis_command_observation""#,
            r#"{"kind":null}"#,
            r#"{"kind":"metis_command_observation_extra"}"#,
        ]
    });
    let actual = tracing::info_span!("act").in_scope(|| originals.map(is_observation));
    tracing::info_span!("assert").in_scope(|| assert_eq!(actual, [false; 5]));
}

#[test]
fn independent_keyboard_flags_preserve_named_wire_and_decoded_values() {
    let _trace = trace();
    let _test = tracing::info_span!("command_observation_independent_flags").entered();
    // Six independent WebIDL Boolean observations form a Cartesian domain of 2^6 values.
    for mask in 0_u8..(1 << 6) {
        let _vector = tracing::info_span!("flag_vector", mask).entered();
        let (flags, expected) = tracing::info_span!("arrange").in_scope(|| {
            let flags: [bool; 6] = std::array::from_fn(|bit| mask & (1 << bit) != 0);
            let [ctrl, alt, shift, meta, repeat, trusted] = flags;
            let event = Event::MetadataDecision {
                keyboard: Some(KeyboardSnapshot {
                    key: "患者-🦀-q".into(),
                    code: "KeyQ".into(),
                    ctrl,
                    alt,
                    shift,
                    meta,
                    repeat,
                    trusted,
                }),
                admission: Admission::Ready,
                decision: Decision::Ignored,
            };
            let record = Record::new(
                identity(7),
                SequenceId::new(13).expect("positive sequence"),
                Some(SequenceId::new(11).expect("positive invocation")),
                event,
            )
            .expect("independent observed metadata");
            (flags, record)
        });
        let (wire, decoded) = tracing::info_span!("act").in_scope(|| {
            let encoded = encode_observation(&expected, MAX_BYTES).expect("bounded flags");
            let wire: serde_json::Value = serde_json::from_str(&encoded).expect("actual wire");
            let decoded = decode_observation(&encoded, MAX_BYTES).expect("decoded flags");
            (wire, decoded)
        });
        tracing::info_span!("assert").in_scope(|| {
            let names = ["ctrl", "alt", "shift", "meta", "repeat", "trusted"];
            for (name, expected_flag) in names.into_iter().zip(flags) {
                assert_eq!(wire["event"]["keyboard"][name], expected_flag, "{name}");
            }
            let Event::MetadataDecision {
                keyboard: Some(keyboard),
                ..
            } = decoded.event()
            else {
                panic!("decoded keyboard metadata is absent");
            };
            assert_eq!(keyboard.ctrl, flags[0]);
            assert_eq!(keyboard.alt, flags[1]);
            assert_eq!(keyboard.shift, flags[2]);
            assert_eq!(keyboard.meta, flags[3]);
            assert_eq!(keyboard.repeat, flags[4]);
            assert_eq!(keyboard.trusted, flags[5]);
            assert_eq!(keyboard.key, "患者-🦀-q");
            assert_eq!(keyboard.code, "KeyQ");
            assert_eq!(decoded, expected);
        });
    }
}
