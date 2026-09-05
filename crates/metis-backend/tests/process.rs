//! Real executable sessions. The committed gate builds both binaries first.
use std::process::Command;

fn execute(arguments: &[&str]) -> (std::process::ExitStatus, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_metis-backend"))
        .args(arguments)
        .output()
        .expect("launch backend process");
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
fn separate_executables_compute_changed_form_values() {
    for (weight, expected) in [("60", 0.36_f64), ("80", 0.48_f64)] {
        let (status, report) = execute(&[weight, "2", "0.2"]);
        assert!(status.success(), "{report}");
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
        let gamma = 6.0 * f64::EPSILON / (1.0 - 6.0 * f64::EPSILON);
        assert!((rate - expected).abs() <= expected * gamma);
        assert!(report.contains("2 audit records verified"), "{report}");
    }
}

#[test]
fn invalid_form_input_cannot_report_a_result() {
    let (status, report) = execute(&["NaN", "2", "0.2"]);
    assert!(!status.success());
    assert!(!report.contains("rate_ml_hr="));
    // The IPC error contract exposes the stable code, not rejected input text.
    assert!(report.contains("ERR_NUMERIC_INSTABILITY"), "{report}");
}
