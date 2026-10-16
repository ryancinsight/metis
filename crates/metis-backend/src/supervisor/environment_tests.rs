//! Real two-hop environment law at the supervisor boundary.
//!
//! The outer reexec receives controlled native values. Its child launches a
//! leaf through the unchanged production `process_spec`: Runtime must retain
//! the explicit startup selection and admitted OS path; Isolated retains
//! neither. Both policies exclude PATH, a token-like fixture key and the
//! compile-only inventory key. Empty, malformed JSON and non-Unicode values
//! cross this transport unchanged; the `WebView` selection decoder owns their
//! later semantic rejection. Expectations travel stdin, never an admitted
//! helper environment key. No process-global environment is mutated.
//!
//! Each contained child has a five-second wait and the existing one-second
//! cleanup bound. The outer parent has twice that wait, covering the actual
//! inner computation and cleanup. A reader drains stdout concurrently with
//! each finite wait, preventing pipe backpressure. Every wait outcome retires
//! the owned tree before joining that reader. These tests observe native process delivery, not CDP
//! execution or page readiness.

use super::*;
use std::{
    ffi::{OsStr, OsString},
    io::{Read, Write},
    path::PathBuf,
};

const SELECTION: &str = "MOIRAI_WEBVIEW_STARTUP_OBSERVATION";
const TOKEN: &str = "METIS_ENVIRONMENT_TEST_TOKEN";
const INVENTORY: &str = "METIS_WEBVIEW_ASSET_INVENTORY";
const EXCLUDED: [&str; 3] = ["PATH", TOKEN, INVENTORY];
const MARKERS: [&str; 3] = [
    "excluded search path",
    "fixture-only token λ🦀",
    "compile-only inventory",
];
const PARENT: &str = "supervisor::environment_tests::environment_parent_entry";
const LEAF: &str = "supervisor::environment_tests::environment_leaf_entry";
const CHILD_WAIT: Duration = Duration::from_secs(5);

fn trace() -> tracing::subscriber::DefaultGuard {
    tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_test_writer()
            .without_time()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .finish(),
    )
}

fn executable() -> PathBuf {
    std::env::current_exe().expect("actual reexec binary")
}

fn runtime_path() -> OsString {
    executable()
        .parent()
        .expect("actual executable directory")
        .as_os_str()
        .to_owned()
}

fn arguments(entry: &str) -> [String; 5] {
    [
        "--ignored",
        "--exact",
        entry,
        "--nocapture",
        "--test-threads=1",
    ]
    .map(str::to_owned)
}

fn native_invalid() -> OsString {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(vec![b'x', 0x80, 0xff])
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        OsString::from_wide(&[0x0078, 0xd800, 0x0079])
    }
}

fn wire(policy: ProcessEnvironment, selection: Option<&[u8]>) -> Vec<u8> {
    let mut result = vec![
        u8::from(policy == ProcessEnvironment::Runtime),
        u8::from(selection.is_some()),
    ];
    if let Some(selection) = selection {
        result.extend_from_slice(selection);
    }
    result
}

fn expected() -> (ProcessEnvironment, Option<Vec<u8>>) {
    let mut input = Vec::new();
    std::io::stdin()
        .read_to_end(&mut input)
        .expect("actual parent expectation pipe");
    assert!(input.len() >= 2, "two actual presence flags");
    let policy = match input[0] {
        0 => ProcessEnvironment::Isolated,
        1 => ProcessEnvironment::Runtime,
        value => panic!("undeclared process policy {value}"),
    };
    let selection = match input[1] {
        0 => {
            assert_eq!(input.len(), 2, "absent value has no payload");
            None
        }
        1 => Some(input[2..].to_owned()),
        value => panic!("undeclared presence flag {value}"),
    };
    (policy, selection)
}

fn native_selection() -> Option<Vec<u8>> {
    std::env::var_os(SELECTION).map(|value| value.as_encoded_bytes().to_owned())
}

fn exchange(spec: ProcessSpec, input: &[u8], timeout: Duration) -> (ProcessStatus, String) {
    let mut child = ProcessSupervisor::new()
        .spawn(spec, ProcessDropPolicy::TerminateOnDrop)
        .expect("actual contained reexec");
    let mut stdin = child.take_stdin().expect("owned expectation writer");
    let mut stdout = child.take_stdout().expect("owned child output");
    let reader = std::thread::spawn(move || {
        let mut output = String::new();
        stdout.read_to_string(&mut output).map(|_| output)
    });
    let written = stdin.write_all(input);
    drop(stdin);
    let waited = child.wait_timeout(timeout);
    let cleanup = child.terminate_timeout(CLEANUP_DEADLINE);
    drop(child);
    let captured = reader.join();
    let context =
        format!("write={written:?}; wait={waited:?}; cleanup={cleanup:?}; output={captured:?}");
    written.expect(&context);
    cleanup.expect(&context);
    let status = waited.expect(&context).expect(&context);
    let output = captured.expect(&context).expect(&context);
    (status, output)
}

#[test]
fn runtime_selection_crosses_the_real_supervised_hop_without_other_application_values() {
    let _trace = trace();
    let _test = tracing::info_span!("supervised_native_environment").entered();
    let cases = tracing::info_span!("arrange").in_scope(|| {
        [
            ("unicode", Some(OsString::from(r##"{"start_uri":"https://metis.invalid/index.html","selectors":["#状態-🦀","#result"]}"##))),
            ("absent", None),
            ("empty", Some(OsString::new())),
            ("malformed_json", Some(OsString::from("{not JSON 患者"))),
            ("native_non_unicode", Some(native_invalid())),
        ]
    });
    for (name, selection) in cases {
        for policy in [ProcessEnvironment::Runtime, ProcessEnvironment::Isolated] {
            let _case = tracing::info_span!("case", name, ?policy).entered();
            let (spec, input) = tracing::info_span!("arrange").in_scope(|| {
                let mut spec = ProcessSpec::new(executable())
                    .args(arguments(PARENT))
                    .env_clear()
                    .piped_stdio()
                    .tree_containment();
                for key in RUNTIME_ENVIRONMENT_KEYS {
                    if key == SELECTION {
                        continue;
                    }
                    if let Some(value) = std::env::var_os(key) {
                        spec = spec.env(key, value);
                    }
                }
                spec = spec.env("TEMP", runtime_path());
                for (key, marker) in EXCLUDED.into_iter().zip(MARKERS) {
                    spec = spec.env(key, marker);
                }
                if let Some(value) = &selection {
                    spec = spec.env(SELECTION, value);
                }
                let input = wire(policy, selection.as_deref().map(OsStr::as_encoded_bytes));
                (spec, input)
            });
            let (status, output) =
                tracing::info_span!("act").in_scope(|| exchange(spec, &input, CHILD_WAIT * 2));
            tracing::info_span!("assert").in_scope(|| {
                assert_eq!(status.code, Some(0), "actual two-hop output: {output}");
                assert!(output.contains("1 passed; 0 failed; 0 ignored"), "{output}");
            });
        }
    }
}

#[test]
#[ignore = "actual controlled outer process invokes this two-hop entry"]
fn environment_parent_entry() {
    let _trace = trace();
    let _test = tracing::info_span!("actual_environment_parent").entered();
    let (policy, selection) = tracing::info_span!("arrange").in_scope(expected);
    tracing::info_span!("assert", phase = "first_hop").in_scope(|| {
        assert_eq!(native_selection(), selection);
        for (key, marker) in EXCLUDED.into_iter().zip(MARKERS) {
            assert_eq!(std::env::var_os(key), Some(OsString::from(marker)));
        }
    });
    let (spec, input) = tracing::info_span!("arrange").in_scope(|| {
        let spec = process_spec(&executable(), &arguments(LEAF), policy);
        let selected = if policy == ProcessEnvironment::Runtime {
            selection.as_deref()
        } else {
            None
        };
        (spec, wire(policy, selected))
    });
    let (status, output) =
        tracing::info_span!("act").in_scope(|| exchange(spec, &input, CHILD_WAIT));
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(
            status.code,
            Some(0),
            "actual supervised leaf output: {output}"
        );
        assert!(output.contains("1 passed; 0 failed; 0 ignored"), "{output}");
    });
}

#[test]
#[ignore = "actual supervisor process_spec invokes this environment leaf"]
fn environment_leaf_entry() {
    let _trace = trace();
    let _test = tracing::info_span!("actual_environment_leaf").entered();
    let (policy, selection) = tracing::info_span!("arrange").in_scope(expected);
    let (actual, temporary, excluded) = tracing::info_span!("act").in_scope(|| {
        (
            native_selection(),
            std::env::var_os("TEMP"),
            EXCLUDED.map(std::env::var_os),
        )
    });
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(
            actual, selection,
            "native startup selection must survive exactly"
        );
        assert_eq!(
            temporary,
            (policy == ProcessEnvironment::Runtime).then(runtime_path)
        );
        assert_eq!(excluded, [None, None, None]);
    });
}
