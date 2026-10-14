use super::{TARGET, with_writer};
use serde_json::Value;
use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

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
                assert!(
                    !values
                        .iter()
                        .any(|value| value["fields"]["event"] == "unrelated")
                );
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
