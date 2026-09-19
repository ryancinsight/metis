use super::*;
use metis_core::capability::{CapabilityGrantSpec, CapabilityScope};
use metis_core::error::ErrorCode;
use metis_core::host::{HostOrigin, HostPolicy, HostSessionId, WindowId};
use std::io::{Read, Write};

const KEY: &[u8] = b"metis-scoped-process-test-key";

fn capability(
    scope: CapabilityScope,
) -> Result<VerifiedHostCapability<{ CapabilityScope::RUN_PROCESS.0 }>, metis_core::error::MetisError>
{
    let policy = HostPolicy::new(
        HostOrigin::parse("http://127.0.0.1:8080").expect("test origin"),
        WindowId::new(1).expect("test window"),
    );
    let session = HostSessionId::new([9; 16]).expect("test session");
    let context = policy.context_for(session);
    let token = context.issue_capability(
        CapabilityGrantSpec {
            token_id: 9,
            principal_id: session.as_bytes(),
            scope,
            issued_at_secs: 10,
            duration_secs: 100,
            nonce: 9,
        },
        KEY,
    )?;
    policy.authorize::<{ CapabilityScope::RUN_PROCESS.0 }>(&token, &context, 11, KEY)
}

fn fixture_provider() -> ScopedProcessProvider {
    ScopedProcessProvider::new(
        std::env::current_exe().expect("test binary"),
        ProcessContainment::DirectChild,
        [
            OsString::from("--exact"),
            OsString::from("scoped_process::tests::child_success"),
            OsString::from("scoped_process::tests::child_blocking"),
            OsString::from("scoped_process::tests::child_stderr"),
            OsString::from("--nocapture"),
        ],
    )
    .expect("provider")
    .with_environment([("METIS_SCOPED_PROCESS_CHILD", "1")])
    .expect("provider environment")
}

fn child_invocation() -> bool {
    std::env::var_os("METIS_SCOPED_PROCESS_CHILD").is_some()
}

#[test]
fn child_success() {
    if child_invocation() {
        std::io::stdout()
            .write_all(b"process-output")
            .expect("child stdout");
    }
}

#[test]
fn child_stderr() {
    if child_invocation() {
        std::io::stderr()
            .write_all(b"secret-token")
            .expect("child stderr");
    }
}

#[test]
fn child_blocking() {
    if child_invocation() {
        let mut byte = [0u8; 1];
        std::io::stdin()
            .read_exact(&mut byte)
            .expect("parent closes stdin at cleanup");
    }
}

#[test]
fn runs_allowlisted_process_and_redacts_stderr_content() {
    let capability = capability(CapabilityScope::RUN_PROCESS).expect("capability");
    let provider = fixture_provider();
    let output = provider
        .run(
            &capability,
            [
                "--exact",
                "scoped_process::tests::child_stderr",
                "--nocapture",
            ],
            Duration::from_secs(5),
        )
        .expect("process output");
    assert!(!String::from_utf8_lossy(output.stdout()).contains("secret-token"));
    assert_eq!(output.stderr_bytes(), b"secret-token".len());
    assert_eq!(output.outcome(), ProcessOutcome::Succeeded);
    assert!(!format!("{output:?}").contains("secret-token"));
}

#[test]
fn returns_real_stdout_and_rejects_invalid_deadline_or_program() {
    let capability = capability(CapabilityScope::RUN_PROCESS).expect("capability");
    let provider = fixture_provider();
    let output = provider
        .run(
            &capability,
            [
                "--exact",
                "scoped_process::tests::child_success",
                "--nocapture",
            ],
            Duration::from_secs(5),
        )
        .expect("process output");
    assert!(String::from_utf8_lossy(output.stdout()).contains("process-output"));
    assert_eq!(output.outcome(), ProcessOutcome::Succeeded);
    assert!(matches!(
        provider.run(&capability, std::iter::empty::<OsString>(), Duration::ZERO,),
        Err(ScopedProcessError::InvalidDeadline)
    ));
    assert!(matches!(
        ScopedProcessProvider::new(
            "relative.exe",
            ProcessContainment::DirectChild,
            std::iter::empty::<OsString>(),
        ),
        Err(ScopedProcessError::InvalidProgram)
    ));
}

#[test]
fn rejects_arguments_and_missing_scope() {
    let provider = fixture_provider();
    let process_capability = capability(CapabilityScope::RUN_PROCESS).expect("capability");
    assert!(matches!(
        provider.run(
            &process_capability,
            ["--unsupported"],
            Duration::from_secs(1)
        ),
        Err(ScopedProcessError::ArgumentDenied)
    ));
    let denied = capability(CapabilityScope::UI_RENDER).expect_err("scope denial");
    assert_eq!(denied.code, ErrorCode::InsufficientScope);
}

#[test]
fn deadline_terminates_a_real_child() {
    let capability = capability(CapabilityScope::RUN_PROCESS).expect("capability");
    let provider = fixture_provider();
    let error = provider
        .run(
            &capability,
            [
                "--exact",
                "scoped_process::tests::child_blocking",
                "--nocapture",
            ],
            Duration::from_millis(10),
        )
        .expect_err("deadline");
    assert!(matches!(error, ScopedProcessError::DeadlineExceeded));
}
