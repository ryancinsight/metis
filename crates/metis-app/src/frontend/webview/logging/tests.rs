use super::{TARGET, with_writer};
use serde_json::Value;
use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

const LATE_TARGET: &str = "moirai_pal::windows::webview::late_input";
const COMMAND_TARGET: &str = "metis_app::webview::command_observation";
const COMMAND_EVENT: &str = "page_command_observation";

struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|error| io::Error::other(error.to_string()))?
            .extend_from_slice(bytes);
        // Nextest retains stderr for failures, including the full span tree.
        io::stderr().write_all(bytes)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // Each write publishes its bytes under this lock; flush synchronizes
        // with any writer before reporting that publication is complete.
        drop(
            self.0
                .lock()
                .map_err(|error| io::Error::other(error.to_string()))?,
        );
        io::stderr().flush()
    }
}

fn records(output: &Mutex<Vec<u8>>) -> Vec<(Value, usize)> {
    let bytes = output.lock().expect("actual trace buffer").clone();
    bytes
        .split_inclusive(|byte| *byte == b'\n')
        .map(|line| {
            assert_eq!(line.last(), Some(&b'\n'));
            (
                serde_json::from_slice(line).expect("actual UTF-8 JSON trace record"),
                line.len(),
            )
        })
        .collect()
}

#[test]
fn only_selected_native_diagnostics_gain_debug_without_losing_normal_context() {
    for selected in [false, true] {
        let output = Arc::new(Mutex::new(Vec::new()));
        let writer = Arc::clone(&output);
        let subscriber = with_writer(move || Capture(Arc::clone(&writer)), selected);
        tracing::subscriber::with_default(subscriber, || {
            let arranged = tracing::info_span!("arrange")
                .in_scope(|| ["native", "🦀", "\"", "\\", "\n"].join(";"));
            tracing::info_span!("act").in_scope(|| {
                tracing::info!(event = "ordinary", value = %arranged);
                tracing::debug!(target: TARGET, parent: None,
                    event = "webview_startup_observation", observation = %arranged);
                tracing::info!(target: LATE_TARGET, parent: None,
                    event = "webview_late_input_observation", observation = %arranged);
                tracing::debug!(target: LATE_TARGET, event = "late_unrelated_debug");
                tracing::info!(target: COMMAND_TARGET, parent: None,
                    event = COMMAND_EVENT, observation = %arranged);
                tracing::debug!(target: COMMAND_TARGET, event = "command_unrelated_debug");
                tracing::debug!(target: "metis_unrelated_debug", event = "unrelated");
            });
            tracing::info_span!("assert").in_scope(|| {
                let values: Vec<_> = records(&output)
                    .into_iter()
                    .map(|(value, _)| value)
                    .collect();
                let ordinary: Vec<_> = values
                    .iter()
                    .filter(|value| value["fields"]["event"] == "ordinary")
                    .collect();
                assert_eq!(ordinary.len(), 1);
                assert_eq!(ordinary[0]["fields"]["value"], arranged);
                assert_eq!(ordinary[0]["span"]["name"], "act");
                assert_eq!(ordinary[0]["spans"][0]["name"], "act");
                assert!(!values.iter().any(|value| matches!(
                    value["fields"]["event"].as_str(),
                    Some("unrelated" | "late_unrelated_debug" | "command_unrelated_debug")
                )));
                let diagnostics: Vec<_> = values
                    .iter()
                    .filter(|value| value["fields"]["event"] == "webview_startup_observation")
                    .collect();
                assert_eq!(diagnostics.len(), usize::from(selected));
                for diagnostic in diagnostics {
                    assert_eq!(diagnostic["fields"]["observation"], arranged);
                    assert_eq!(diagnostic["level"], "DEBUG");
                    assert_eq!(diagnostic["target"], TARGET);
                    assert_eq!(diagnostic.get("span"), None);
                    assert_eq!(diagnostic.get("spans"), None);
                    assert_eq!(diagnostic.get("timestamp"), None);
                }
                let late: Vec<_> = values
                    .iter()
                    .filter(|value| value["fields"]["event"] == "webview_late_input_observation")
                    .collect();
                assert_eq!(late.len(), usize::from(selected));
                for diagnostic in late {
                    assert_eq!(diagnostic["fields"]["observation"], arranged);
                    assert_eq!(diagnostic["level"], "INFO");
                    assert_eq!(diagnostic["target"], LATE_TARGET);
                    assert_eq!(diagnostic.get("span"), None);
                    assert_eq!(diagnostic.get("spans"), None);
                    assert_eq!(diagnostic.get("timestamp"), None);
                }
                let commands: Vec<_> = values
                    .iter()
                    .filter(|value| value["fields"]["event"] == COMMAND_EVENT)
                    .collect();
                assert_eq!(commands.len(), usize::from(selected));
                for diagnostic in commands {
                    assert_eq!(diagnostic["fields"]["observation"], arranged);
                    assert_eq!(diagnostic["level"], "INFO");
                    assert_eq!(diagnostic["target"], COMMAND_TARGET);
                    assert_eq!(diagnostic.get("span"), None);
                    assert_eq!(diagnostic.get("spans"), None);
                    assert_eq!(diagnostic.get("timestamp"), None);
                }
            });
        });
    }
}

#[test]
fn diagnostic_envelopes_preserve_values_and_have_bounded_json_escape_overhead() {
    let output = Arc::new(Mutex::new(Vec::new()));
    let writer = Arc::clone(&output);
    let subscriber = with_writer(move || Capture(Arc::clone(&writer)), true);
    tracing::subscriber::with_default(subscriber, || {
        let inputs = tracing::info_span!("arrange").in_scope(|| {
            let controls: String = ('\u{0}'..='\u{1f}').collect();
            let count = controls.chars().count();
            vec![
                String::new(),
                "native".into(),
                "\"\\".repeat(count),
                controls,
                "λ🦀".repeat(count),
            ]
        });
        tracing::info_span!("act").in_scope(|| {
            for input in &inputs {
                tracing::debug!(target: TARGET, parent: None,
                    event = "webview_startup_observation", observation = %input);
            }
        });
        tracing::info_span!("assert").in_scope(|| {
            let values = records(&output);
            let diagnostics: Vec<_> = values
                .iter()
                .filter(|(value, _)| value["fields"]["event"] == "webview_startup_observation")
                .collect();
            assert_eq!(diagnostics.len(), inputs.len());
            let empty = serde_json::json!({
                "level": "DEBUG",
                "fields": {"event": "webview_startup_observation", "observation": ""},
                "target": TARGET,
            });
            assert_eq!(diagnostics[0].0, empty);
            // The agreed physical envelope includes the formatter's LF.
            let envelope_bytes = concat!(
                r#"{"level":"DEBUG","fields":{"event":"webview_startup_observation","observation":""},"target":"moirai_pal::windows::webview::startup"}"#,
                "\n"
            ).len();
            assert_eq!(diagnostics[0].1, envelope_bytes);
            for (record, input) in diagnostics.iter().zip(&inputs) {
                assert_eq!(record.0["fields"]["observation"], *input);
                // RFC 8259 §7: a one-byte control can expand to six ASCII
                // bytes (\uXXXX); every other scalar uses at most that ratio.
                assert!(record.1 <= envelope_bytes + 6 * input.len());
            }
        });
    });
}

#[test]
fn compact_provider_json_obeys_the_aggregate_double_escape_bound() {
    let output = Arc::new(Mutex::new(Vec::new()));
    let writer = Arc::clone(&output);
    let subscriber = with_writer(move || Capture(Arc::clone(&writer)), true);
    tracing::subscriber::with_default(subscriber, || {
        let encoded = tracing::info_span!("arrange").in_scope(|| {
            let controls: String = ('\u{0}'..='\u{1f}').collect();
            [String::new(), "\"\\".into(), controls, "Native λ🦀".into()]
                .map(|value| serde_json::json!({"kind":"dom_status", "value":value}).to_string())
        });
        tracing::info_span!("act").in_scope(|| {
            for observation in &encoded {
                tracing::debug!(target: TARGET, parent: None,
                    event = "webview_startup_observation", observation = %observation);
            }
        });
        tracing::info_span!("assert").in_scope(|| {
            let observations: Vec<_> = records(&output)
                .into_iter()
                .filter(|(value, _)| value["fields"]["event"] == "webview_startup_observation")
                .collect();
            assert_eq!(observations.len(), encoded.len());
            for ((record, physical_bytes), encoded) in observations.iter().zip(encoded) {
                assert_eq!(record["fields"]["observation"], encoded);
                // Compact JSON contains no raw C0 controls. A second JSON
                // string layer only doubles quote/backslash bytes; all other
                // UTF-8 bytes retain their length. The empty wire envelope,
                // including its LF, has the separately verified 133 bytes.
                assert!(!encoded.bytes().any(|byte| byte < b' '));
                assert!(*physical_bytes <= 133 + 2 * encoded.len());
            }
        });
    });
}

#[test]
fn late_controller_wire_preserves_compact_json_with_a_fixed_parentless_info_envelope() {
    const EVENT: &str = "webview_late_input_observation";
    let output = Arc::new(Mutex::new(Vec::new()));
    let writer = Arc::clone(&output);
    let subscriber = with_writer(move || Capture(Arc::clone(&writer)), true);
    tracing::subscriber::with_default(subscriber, || {
        let encoded = tracing::info_span!("arrange").in_scope(|| {
            let controls: String = ('\u{0}'..='\u{1f}').collect();
            [String::new(), "\"\\".into(), controls, "Native λ🦀".into()].map(|value| {
                serde_json::json!({"kind":"controller_input", "value":value}).to_string()
            })
        });
        tracing::info_span!("act", context = "ordinary span remains outside the wire").in_scope(
            || {
                for observation in &encoded {
                    tracing::info!(target: LATE_TARGET, parent: None,
                        event = EVENT, observation = %observation);
                }
            },
        );
        tracing::info_span!("assert").in_scope(|| {
            let observations: Vec<_> = records(&output)
                .into_iter()
                .filter(|(value, _)| value["fields"]["event"] == EVENT)
                .collect();
            assert_eq!(observations.len(), encoded.len());
            let envelope = concat!(
                r#"{"level":"INFO","fields":{"event":"webview_late_input_observation","observation":""},"target":"moirai_pal::windows::webview::late_input"}"#,
                "\n"
            )
            .len();
            assert_eq!(envelope, 138);
            // The provider separately bounds compact bodies at 65,536 bytes
            // and 256 records; its two terminal bodies are at most 534 bytes.
            // The second JSON layer expands each compact byte at most twice.
            let suffix_bound = 2 * 65_536 + 4 * 534 + envelope * (256 + 2);
            assert_eq!(suffix_bound, 168_812);
            assert_eq!(427_530 + suffix_bound, 596_342);
            assert!(427_530 + suffix_bound < 1024 * 1024);
            for ((record, physical_bytes), encoded) in observations.iter().zip(encoded) {
                assert_eq!(record["fields"]["observation"], encoded);
                assert_eq!(record["level"], "INFO");
                assert_eq!(record["target"], LATE_TARGET);
                assert_eq!(record.get("span"), None);
                assert_eq!(record.get("spans"), None);
                assert_eq!(record.get("timestamp"), None);
                // Compact JSON has no raw C0 controls; only quotes and
                // backslashes expand in the second string encoding.
                assert!(!encoded.bytes().any(|byte| byte < b' '));
                assert!(*physical_bytes <= envelope + 2 * encoded.len());
                let escaped = serde_json::to_string(&encoded).expect("actual second JSON layer");
                assert_eq!(*physical_bytes, envelope + escaped.len() - 2);
            }
        });
    });
}

#[test]
fn document_observation_preserves_valid_json_and_all_three_streams_fit_the_cap() {
    use metis_core::embedded_assets::Digest;
    use metis_web::embedded::command_observation::{
        Event, MAX_BYTES, MAX_RECORDS, Record, encode_observation,
    };
    use metis_web::embedded::{PageIdentity, PageRole, SequenceId};
    let output = Arc::new(Mutex::new(Vec::new()));
    let writer = Arc::clone(&output);
    let subscriber = with_writer(move || Capture(Arc::clone(&writer)), true);
    tracing::subscriber::with_default(subscriber, || {
        let inputs = tracing::info_span!("arrange").in_scope(|| {
            let identity = PageIdentity::new(
                PageRole::Form,
                SequenceId::new(7).expect("mount"),
                Digest::hash(b"real formatter observation module"),
            );
            let compact = encode_observation(
                &Record::new(
                    identity,
                    SequenceId::new(1).expect("record"),
                    None,
                    Event::Selected,
                )
                .expect("valid record"),
                MAX_BYTES,
            )
            .expect("actual closed codec");
            let object: Value = serde_json::from_str(&compact).expect("record JSON");
            let formatted = format!(
                "\t{}\r\n",
                serde_json::to_string_pretty(&object).expect("equivalent JSON formatting")
            );
            let unicode = serde_json::json!({"key":"患者-🦀\r\n\t\"\\", "code":"KeyS"}).to_string();
            [compact, formatted, unicode]
        });
        tracing::info_span!(
            "act",
            context = "ordinary context stays outside this envelope"
        )
        .in_scope(|| {
            for observation in &inputs {
                tracing::info!(target: COMMAND_TARGET, parent: None,
                        event=COMMAND_EVENT, observation=%observation);
            }
        });
        tracing::info_span!("assert").in_scope(|| {
            let envelope = serde_json::to_vec(&serde_json::json!({
                "level":"INFO", "fields":{"event":COMMAND_EVENT,"observation":""},
                "target":COMMAND_TARGET,
            }))
            .expect("actual empty envelope")
            .len()
                + 1;
            let actual: Vec<_> = records(&output)
                .into_iter()
                .filter(|(record, _)| record["fields"]["event"] == COMMAND_EVENT)
                .collect();
            assert_eq!(actual.len(), inputs.len());
            for ((record, physical), input) in actual.iter().zip(&inputs) {
                assert_eq!(record["fields"]["observation"], *input);
                assert_eq!(record.get("span"), None);
                assert_eq!(record.get("spans"), None);
                assert_eq!(record.get("timestamp"), None);
                let escaped = serde_json::to_string(input).expect("actual string encoding");
                assert_eq!(*physical, envelope + escaped.len() - 2);
                // Valid JSON permits raw C0 only as tab/LF/CR whitespace;
                // those and quote/backslash all expand by at most twofold.
                assert!(
                    input
                        .bytes()
                        .filter(|byte| *byte < b' ')
                        .all(|byte| matches!(byte, b'\t' | b'\n' | b'\r'))
                );
                assert!(*physical <= envelope + 2 * input.len());
            }
            let added = 2 * MAX_BYTES + MAX_RECORDS * envelope;
            let combined = 596_342 + added;
            assert!(combined < 1024 * 1024);
            tracing::info!(
                envelope,
                added,
                combined,
                "Actual three-stream formatter bound"
            );
        });
    });
}
