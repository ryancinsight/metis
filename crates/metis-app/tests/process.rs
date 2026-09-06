//! Real sessions prove one copied image suffices for two process roles.
use metis_core::protocol::{HandshakeRequestPayload, MessageType, PROTOCOL_VERSION};
use metis_ipc::{IpcTransport, StreamTransport};
use std::process::Command;

fn execute(arguments: &[&str]) -> (std::process::ExitStatus, String) {
    execute_at(
        std::path::Path::new(env!("CARGO_BIN_EXE_metis-app")),
        arguments,
    )
}

fn execute_at(
    executable: &std::path::Path,
    arguments: &[&str],
) -> (std::process::ExitStatus, String) {
    let output = Command::new(executable)
        .current_dir(executable.parent().expect("application image directory"))
        .args(arguments)
        .output()
        .expect("launch application process");
    assert!(
        output.stdout.is_empty(),
        "wire stays on the private child pipes"
    );
    (
        output.status,
        String::from_utf8(output.stderr).expect("UTF-8 diagnostics"),
    )
}

#[test]
fn one_executable_computes_changed_form_values() {
    for (weight, expected) in [("60", 0.36_f64), ("80", 0.48_f64)] {
        let (status, report) = execute(&[weight, "2", "0.2"]);
        assert!(status.success(), "{report}");
        assert_session(&report, expected);
    }
}

fn assert_session(report: &str, expected: f64) {
    let fields: std::collections::HashMap<_, _> = report
        .split_whitespace()
        .filter_map(|field| field.split_once('='))
        .collect();
    let frontend: u32 = fields["frontend_pid"].parse().expect("frontend PID");
    let backend: u32 = fields["backend_pid"].parse().expect("backend PID");
    assert_ne!(frontend, backend);
    assert_ne!(frontend, std::process::id());
    assert_ne!(backend, std::process::id());
    let rate: f64 = fields["rate_ml_hr"].parse().expect("computed rate");
    // Four arithmetic operations, input decimal rounding and oracle rounding
    // are bounded by gamma(6) for these finite, normally represented values.
    let gamma = 6.0 * f64::EPSILON / (1.0 - 6.0 * f64::EPSILON);
    assert!((rate - expected).abs() <= expected * gamma);
    assert!(report.contains("2 audit records verified"), "{report}");
}

#[test]
fn invalid_form_input_cannot_report_a_result() {
    let (status, report) = execute(&["NaN", "2", "0.2"]);
    assert!(!status.success());
    assert!(!report.contains("rate_ml_hr="));
    // The IPC error contract exposes the stable code, not rejected input text.
    assert!(report.contains("ERR_NUMERIC_INSTABILITY"), "{report}");
}

#[test]
fn renamed_image_runs_from_a_directory_without_companions() {
    let directory =
        std::env::temp_dir().join(format!("metis-application-copy-{}", std::process::id()));
    // create_dir refuses existing user/peer paths; only this created directory
    // is removed, after the contained process has exited and closed its image.
    std::fs::create_dir(&directory).expect("unique application test directory");
    let copied = directory.join(format!("viewer{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(env!("CARGO_BIN_EXE_metis-app"), &copied).expect("copy application image");
    let results = [("60", 0.36_f64), ("80", 0.48_f64)].map(|(weight, rate)| {
        let (status, report) = execute_at(&copied, &[weight, "2", "0.2"]);
        (status, report, rate)
    });
    let contents: Vec<_> = std::fs::read_dir(&directory)
        .expect("application directory")
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    std::fs::remove_file(&copied).expect("remove owned image");
    std::fs::remove_dir(&directory).expect("remove empty owned directory");
    assert_eq!(contents, [copied]);
    for (status, report, rate) in results {
        assert!(status.success(), "{report}");
        assert_session(&report, rate);
    }
}

#[test]
fn invalid_role_dispatch_cannot_spawn_a_backend() {
    for arguments in [
        vec![],
        vec!["--unknown", "2", "0.2"],
        vec!["--metis-frontend"],
        vec!["--metis-frontend", "--metis-frontend", "2", "0.2"],
    ] {
        let (status, report) = execute(&arguments);
        assert!(!status.success(), "{report}");
        assert!(!report.contains("backend_pid="), "{report}");
        assert!(!report.contains("rate_ml_hr="), "{report}");
        assert!(report.contains("usage: metis-app"), "{report}");
    }
}

#[test]
fn direct_child_faults_never_construct_parent_authority() {
    for (input, error) in [
        (vec![], "ERR_CONNECTION_CLOSED"),
        (
            vec![0; metis_core::protocol::HEADER_SIZE],
            "ERR_MAGIC_MISMATCH",
        ),
    ] {
        child_failure(&input, error);
    }
}

fn child_failure(input: &[u8], expected_error: &str) {
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_metis-app"))
        .args(["--metis-frontend", "60", "2", "0.2"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("launch isolated child role");
    // Both inputs fit one header. Closing stdin supplies EOF without timing or
    // polling; an invalid magic header fails before payload allocation.
    let child_pid = child.id();
    child
        .stdin
        .take()
        .expect("child input pipe")
        .write_all(input)
        .expect("fault input");
    let output = child.wait_with_output().expect("collect child failure");
    assert!(!output.status.success());
    let report = String::from_utf8(output.stderr).expect("UTF-8 diagnostics");
    assert!(report.contains(expected_error), "{report}");
    assert!(!report.contains("backend_pid="), "{report}");
    assert!(!report.contains("rate_ml_hr="), "{report}");
    // Only a handshake request is emitted, never a parent process session.
    let mut cursor = std::io::Cursor::new(&output.stdout);
    let (header, payload) = StreamTransport::new(&mut cursor, std::io::sink())
        .recv_message()
        .expect("one valid child handshake");
    assert_eq!(header.msg_type, MessageType::HandshakeReq);
    assert_eq!(header.sequence_id, 1);
    assert_eq!(cursor.position(), output.stdout.len() as u64);
    let request = HandshakeRequestPayload::decode(&payload).expect("handshake payload");
    assert_eq!(request.client_version, PROTOCOL_VERSION);
    assert_eq!(request.client_process_id, child_pid);
    assert_ne!(request.client_process_id, std::process::id());
    assert_eq!(
        &request.principal_id[..4],
        &request.client_process_id.to_be_bytes()
    );
}
