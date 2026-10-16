//! The constant is checked against the actual selected-worker Python owner.

use super::trace;
use crate::embedded::command_observation::PROVIDER_SELECTION;
use std::{path::Path, process::Command};

#[test]
fn provider_selection_is_the_actual_python_compact_configuration_bytes() {
    let _trace = trace();
    let (python, scripts, program) = tracing::info_span!("arrange").in_scope(|| {
        let scripts = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent().expect("crates directory").parent().expect("repository root").join("scripts");
        let python = if cfg!(windows) { "python" } else { "python3" };
        let program = "import sys; sys.path.insert(0, sys.argv[1]); from webview_acceptance.scenario import observation_environment; from webview_acceptance.cases import SELECTION_ENV; sys.stdout.write(observation_environment('form', {}, observe_startup=True)[SELECTION_ENV])";
        (python, scripts, program)
    });
    let output = tracing::info_span!("act").in_scope(|| {
        Command::new(python)
            .args(["-I", "-c", program])
            .arg(&scripts)
            .output()
            .expect("real Python selected-worker configuration")
    });
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(
            output.status.code(),
            Some(0),
            "actual Python stderr: {:?}",
            output.stderr
        );
        assert_eq!(output.stderr, Vec::<u8>::new());
        assert_eq!(output.stdout, PROVIDER_SELECTION.as_bytes());
        let value: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("actual Python JSON");
        assert_eq!(value["start_uri"], "https://metis.invalid/index.html");
        assert_eq!(
            value["selectors"],
            serde_json::json!(["#host-status", "#result"])
        );
        assert_eq!(value["late_input_after_key"], 68);
        assert_eq!(
            value
                .as_object()
                .expect("closed selected configuration")
                .len(),
            3
        );
    });
}
