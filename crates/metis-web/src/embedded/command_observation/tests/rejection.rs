//! Original malformed JSON and incoherent typed records fail closed.

use super::super::{
    Admission, Decision, Event, KeyboardSnapshot, MAX_BYTES, Outcome, Record, decode_observation,
    encode_observation,
};
use super::{identity, trace};
use crate::embedded::{PageStage, SequenceId};
use metis_frontend::ApplicationCommand;
use serde_json::{Value, json};
use std::io;

fn encoded(event: Event) -> String {
    let invocation = (!matches!(event, Event::Selected | Event::Quota { .. }))
        .then(|| SequenceId::new(3).expect("invocation"));
    let record = Record::new(
        identity(7),
        SequenceId::new(11).expect("sequence"),
        invocation,
        event,
    )
    .expect("coherent source record");
    encode_observation(&record, MAX_BYTES).expect("bounded source encoding")
}

fn metadata() -> Event {
    Event::MetadataDecision {
        keyboard: Some(KeyboardSnapshot {
            key: "患者-🦀-S".into(),
            code: "KeyS".into(),
            ctrl: false,
            alt: true,
            shift: true,
            meta: false,
            repeat: false,
            trusted: true,
        }),
        admission: Admission::Ready,
        decision: Decision::Command(ApplicationCommand::ThemeSystem),
    }
}

fn changed(original: &str, path: &[&str], replacement: Value) -> String {
    let mut value: Value = serde_json::from_str(original).expect("source wire object");
    let mut current = &mut value;
    for key in path {
        current = &mut current[*key];
    }
    *current = replacement;
    serde_json::to_string(&value).expect("mutated original JSON")
}

fn rejected(originals: &[String]) -> Vec<io::Error> {
    originals
        .iter()
        .map(|original| {
            decode_observation(original, MAX_BYTES).expect_err("malformed original is rejected")
        })
        .collect()
}

fn assert_invalid(errors: Vec<io::Error>, expected_count: usize) {
    assert_eq!(errors.len(), expected_count);
    for error in errors {
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn unknown_fields_and_discriminants_are_rejected_in_every_nested_object() {
    let _trace = trace();
    let _test = tracing::info_span!("command_observation_unknown_schema").entered();
    let malformed = tracing::info_span!("arrange").in_scope(|| {
        let source = encoded(metadata());
        let mut values = Vec::new();
        for path in [
            vec![],
            vec!["identity"],
            vec!["event"],
            vec!["event", "keyboard"],
            vec!["event", "decision"],
        ] {
            let mut value: Value = serde_json::from_str(&source).expect("source object");
            let mut object = &mut value;
            for key in path {
                object = &mut object[key];
            }
            object
                .as_object_mut()
                .expect("actual schema object")
                .insert("undeclared".into(), json!(true));
            values.push(value.to_string());
        }
        for (path, value) in [
            (vec!["kind"], json!("other_observation")),
            (vec!["observation_schema"], json!(2)),
            (vec!["observation_schema"], json!("1")),
            (vec!["event", "type"], json!("undeclared")),
            (vec!["event", "admission"], json!("undeclared")),
            (vec!["event", "decision", "type"], json!("undeclared")),
            (vec!["event", "decision", "command"], json!("command-shell")),
            (vec!["event", "keyboard", "trusted"], json!(1)),
            (vec!["event", "keyboard", "key"], json!(null)),
            (vec!["event", "keyboard", "code"], json!(false)),
        ] {
            values.push(changed(&source, &path, value));
        }
        values.push(changed(
            &encoded(Event::Entered {
                stage: PageStage::Ready,
            }),
            &["event", "stage"],
            json!("undeclared"),
        ));
        values.push(changed(
            &encoded(Event::Returned {
                outcome: Outcome::Ok,
            }),
            &["event", "outcome"],
            json!("undeclared"),
        ));
        for event in [Event::Selected, Event::ClosingIntent] {
            let mut value: Value = serde_json::from_str(&encoded(event)).expect("unit event");
            value["event"]["undeclared"] = json!(true);
            values.push(value.to_string());
        }
        let Event::MetadataDecision { keyboard, .. } = metadata() else {
            unreachable!()
        };
        for decision in [
            Decision::NotEvaluated,
            Decision::Repeat,
            Decision::EscapeMenu,
            Decision::EscapeClose,
            Decision::Ignored,
        ] {
            let evaluated = decision != Decision::NotEvaluated;
            let event = Event::MetadataDecision {
                keyboard: evaluated.then(|| keyboard.clone().expect("actual keyboard")),
                admission: if evaluated {
                    Admission::Ready
                } else {
                    Admission::NotReady
                },
                decision,
            };
            let mut value: Value = serde_json::from_str(&encoded(event)).expect("unit decision");
            value["event"]["decision"]["undeclared"] = json!(true);
            values.push(value.to_string());
        }
        values
    });
    let errors = tracing::info_span!("act").in_scope(|| rejected(&malformed));
    tracing::info_span!("assert").in_scope(|| assert_invalid(errors, 24));
}

#[test]
fn duplicate_original_json_fields_are_rejected_before_any_value_reserialization() {
    let _trace = trace();
    let _test = tracing::info_span!("command_observation_duplicate_original_json").entered();
    let malformed = tracing::info_span!("arrange").in_scope(|| {
        let source = encoded(metadata());
        let replacements = [
            (
                "\"kind\":",
                r#""ki\u006ed":"metis_command_observation","kind":"#,
            ),
            ("\"sequence\":", "\"sequence\":\"11\",\"sequence\":"),
            ("\"invocation\":", "\"invocation\":\"3\",\"invocation\":"),
            ("\"schema\":", "\"schema\":1,\"schema\":"),
            ("\"keyboard\":", "\"keyboard\":null,\"keyboard\":"),
            ("\"key\":", "\"key\":\"duplicate\",\"key\":"),
            (
                "\"command\":",
                "\"command\":\"command-theme-system\",\"command\":",
            ),
            ("\"type\":", "\"type\":\"metadata_decision\",\"type\":"),
            ("\"admission\":", "\"admission\":\"ready\",\"admission\":"),
            ("\"decision\":{", "\"decision\":{\"type\":\"command\","),
        ];
        replacements.map(|(needle, replacement)| {
            let original = source.replacen(needle, replacement, 1);
            assert_ne!(
                original, source,
                "duplicate mutation must reach its wire field"
            );
            original
        })
    });
    let errors = tracing::info_span!("act").in_scope(|| rejected(&malformed));
    tracing::info_span!("assert").in_scope(|| assert_invalid(errors, 10));
}

#[test]
fn malformed_canonical_identity_sequence_invocation_and_counter_text_fail() {
    let _trace = trace();
    let _test = tracing::info_span!("command_observation_integer_and_identity_rejection").entered();
    let malformed = tracing::info_span!("arrange").in_scope(|| {
        let source = encoded(Event::Entered {
            stage: PageStage::Loading,
        });
        let mut values = Vec::new();
        for path in [
            vec!["identity", "mount_id"],
            vec!["sequence"],
            vec!["invocation"],
        ] {
            for text in [
                "",
                "0",
                "01",
                "+1",
                "-1",
                " 1",
                "1 ",
                "18446744073709551616",
                "١",
            ] {
                values.push(changed(&source, &path, json!(text)));
            }
            for value in [json!(1), json!(false), json!(null)] {
                values.push(changed(&source, &path, value));
            }
        }
        for (path, value) in [
            (vec!["identity", "schema"], json!(2)),
            (vec!["identity", "role"], json!("FORM")),
            (vec!["identity", "module_digest"], json!("f".repeat(63))),
            (vec!["identity", "module_digest"], json!("F".repeat(64))),
            (vec!["identity", "module_digest"], json!("g".repeat(64))),
        ] {
            values.push(changed(&source, &path, value));
        }
        let quota = encoded(Event::Quota {
            prior_records: u32::MAX,
            prior_bytes: u32::MAX,
            omitted_at_notice: u64::MAX,
            failed: false,
        });
        for text in ["", "01", "+1", "-1", " 1", "1 ", "18446744073709551616"] {
            values.push(changed(
                &quota,
                &["event", "omitted_at_notice"],
                json!(text),
            ));
        }
        for field in ["prior_records", "prior_bytes"] {
            for value in [json!(-1), json!(u64::from(u32::MAX) + 1), json!("1")] {
                values.push(changed(&quota, &["event", field], value));
            }
        }
        values
    });
    let errors = tracing::info_span!("act").in_scope(|| rejected(&malformed));
    tracing::info_span!("assert").in_scope(|| assert_invalid(errors, 54));
}

#[test]
fn required_invocation_and_admission_options_fail_closed_without_policy_checks() {
    let _trace = trace();
    let _test = tracing::info_span!("command_observation_wire_coherence").entered();
    let malformed = tracing::info_span!("arrange").in_scope(|| {
        let source = encoded(metadata());
        let unready = encoded(Event::MetadataDecision {
            keyboard: None,
            admission: Admission::NotReady,
            decision: Decision::NotEvaluated,
        });
        let mut values = vec![
            changed(&source, &["invocation"], json!(null)),
            changed(&source, &["event", "keyboard"], json!(null)),
            changed(
                &source,
                &["event", "decision"],
                json!({"type":"not_evaluated"}),
            ),
            changed(&source, &["event", "admission"], json!("not_ready")),
            changed(&unready, &["event", "decision"], json!({"type":"ignored"})),
            changed(&encoded(Event::Selected), &["invocation"], json!("3")),
            format!("{source} {{}}"),
        ];
        for original in [&source, &encoded(Event::Selected)] {
            let mut wire: Value = serde_json::from_str(original).expect("source object");
            wire.as_object_mut()
                .expect("envelope object")
                .remove("invocation");
            values.push(wire.to_string());
        }
        values
    });
    let errors = tracing::info_span!("act").in_scope(|| rejected(&malformed));
    tracing::info_span!("assert").in_scope(|| assert_invalid(errors, 9));
}

#[test]
fn typed_constructor_rejects_exact_invocation_and_metadata_shape_violations() {
    let _trace = trace();
    let _test = tracing::info_span!("command_observation_typed_coherence").entered();
    let cases = tracing::info_span!("arrange").in_scope(|| {
        let invocation = Some(SequenceId::new(3).expect("invocation"));
        let mut values = vec![
            (invocation, Event::Selected),
            (None, Event::ClosingIntent),
            (
                None,
                Event::Entered {
                    stage: PageStage::Ready,
                },
            ),
            (
                None,
                Event::Returned {
                    outcome: Outcome::ActionError,
                },
            ),
            (None, metadata()),
            (
                invocation,
                Event::Quota {
                    prior_records: 1,
                    prior_bytes: 1,
                    omitted_at_notice: 1,
                    failed: true,
                },
            ),
        ];
        let Event::MetadataDecision { keyboard, .. } = metadata() else {
            unreachable!()
        };
        for admission in [
            Admission::NotReady,
            Admission::NotKeyboard,
            Admission::MetadataError,
        ] {
            values.push((
                invocation,
                Event::MetadataDecision {
                    keyboard: keyboard.clone(),
                    admission,
                    decision: Decision::NotEvaluated,
                },
            ));
            values.push((
                invocation,
                Event::MetadataDecision {
                    keyboard: None,
                    admission,
                    decision: Decision::Ignored,
                },
            ));
        }
        values.push((
            invocation,
            Event::MetadataDecision {
                keyboard: None,
                admission: Admission::Ready,
                decision: Decision::Ignored,
            },
        ));
        values.push((
            invocation,
            Event::MetadataDecision {
                keyboard,
                admission: Admission::Ready,
                decision: Decision::NotEvaluated,
            },
        ));
        values
    });
    let errors = tracing::info_span!("act").in_scope(|| {
        cases
            .into_iter()
            .map(|(invocation, event)| {
                Record::new(
                    identity(7),
                    SequenceId::new(11).expect("sequence"),
                    invocation,
                    event,
                )
                .expect_err("constructor must reject an incoherent typed observation")
            })
            .collect()
    });
    tracing::info_span!("assert").in_scope(|| assert_invalid(errors, 14));
}
