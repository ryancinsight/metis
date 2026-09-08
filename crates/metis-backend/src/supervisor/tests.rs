//! Supervisor error-path and deadline tests using contained Windows commands.
use super::*;
use metis_core::protocol::{FrameHeader, MessageType};
use std::io::{BufRead, Read, Write};

fn fixture(mode: &str, policy: ProcessDropPolicy) -> ManagedProcess {
    ProcessSupervisor::new()
        .spawn(
            ProcessSpec::new(std::env::current_exe().expect("test executable"))
                .args(["--exact", "supervisor::tests::child_entry", "--nocapture"])
                .env("METIS_SUPERVISOR_FIXTURE", mode)
                .piped_stdio()
                .tree_containment(),
            policy,
        )
        .expect("contained fixture")
}

#[test]
fn child_entry() {
    let Ok(mode) = std::env::var("METIS_SUPERVISOR_FIXTURE") else {
        return;
    };
    match mode.as_str() {
        "wait" => {
            println!("DESCENDANT_READY");
            std::io::stdout().flush().expect("ready signal");
            let mut byte = [0];
            std::io::stdin().read_exact(&mut byte).expect("held pipe");
        }
        "root" => {
            #[expect(
                clippy::zombie_processes,
                reason = "Root intentionally exits first; enclosing test job owns descendant cleanup"
            )]
            let _descendant =
                std::process::Command::new(std::env::current_exe().expect("test executable"))
                    .args(["--exact", "supervisor::tests::child_entry", "--nocapture"])
                    .env("METIS_SUPERVISOR_FIXTURE", "wait")
                    .spawn()
                    .expect("descendant spawn");
        }
        "check" => {
            // Detach disables the inner job's Drop fallback: only watch's
            // explicit cleanup can close this descendant's inherited writer.
            let mut child = fixture("root", ProcessDropPolicy::DetachOnDrop);
            let _input = child.take_stdin().expect("held input");
            let mut output = std::io::BufReader::new(child.take_stdout().expect("output"));
            loop {
                let mut line = String::new();
                assert_ne!(output.read_line(&mut line).expect("ready line"), 0);
                if line.trim() == "DESCENDANT_READY" {
                    break;
                }
            }
            assert_eq!(child.wait().expect("root exit").code, Some(0));
            let (sender, completion) = mpsc::sync_channel(1);
            sender.send(Completion::Closed).expect("completion");
            assert_eq!(
                watch(child, &completion, Instant::now(), SESSION_DEADLINE)
                    .expect("job cleanup")
                    .code,
                Some(0)
            );
            let mut tail = String::new();
            output.read_to_string(&mut tail).expect("descendant EOF");
        }
        _ => panic!("unknown fixture mode"),
    }
}

#[test]
fn closed_session_terminates_descendants_after_root_exit() {
    // The outer kill-on-close job contains a faulty inner implementation too,
    // so a regression fails under a finite deadline without orphaning fixtures.
    let mut child = fixture("check", ProcessDropPolicy::TerminateOnDrop);
    let status = match child.wait_timeout(SESSION_DEADLINE).expect("fixture wait") {
        Some(status) => status,
        None => child
            .terminate_timeout(CLEANUP_DEADLINE)
            .expect("fixture cleanup"),
    };
    assert_eq!(status.code, Some(0));
}

fn shell() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var_os("SystemRoot").expect("Windows system root"))
        .join("System32")
        .join("cmd.exe")
}
fn child(command: &str) -> ManagedProcess {
    ProcessSupervisor::new()
        .spawn(
            ProcessSpec::new(shell())
                .args(["/d", "/c", command])
                .env_clear()
                .piped_stdio()
                .tree_containment(),
            ProcessDropPolicy::TerminateOnDrop,
        )
        .expect("contained fixture")
}
#[test]
fn expired_watchdog_terminates_child_and_reports_timeout() {
    let child = child("set /p INPUT=");
    let (_sender, completion) = mpsc::sync_channel(1);
    let started = Instant::now()
        .checked_sub(SESSION_DEADLINE)
        .expect("test clock interval");
    assert_eq!(
        watch(child, &completion, started, SESSION_DEADLINE)
            .expect_err("deadline")
            .code,
        ErrorCode::Timeout
    );
}
#[test]
fn failed_session_requests_immediate_child_termination() {
    let child = child("set /p INPUT=");
    let (sender, completion) = mpsc::sync_channel(1);
    sender.send(Completion::Failed).expect("completion signal");
    let status =
        watch(child, &completion, Instant::now(), SESSION_DEADLINE).expect("termination confirmed");
    assert_eq!(
        status.outcome,
        moirai_transport::process::ProcessOutcome::Failed
    );
}
#[test]
fn closed_session_preserves_child_exit_status() {
    let child = child("exit 7");
    let (sender, completion) = mpsc::sync_channel(1);
    sender.send(Completion::Closed).expect("completion signal");
    assert_eq!(
        watch(child, &completion, Instant::now(), SESSION_DEADLINE)
            .expect("child exit")
            .code,
        Some(7)
    );
}
#[test]
fn malformed_pipe_input_preserves_protocol_error() {
    struct Handler {
        failure: Option<(metis_ipc::server::FailureContext, ErrorCode)>,
    }
    impl IpcHandler for Handler {
        fn handle_request(&mut self, _: &FrameHeader, _: &[u8]) -> Result<(MessageType, Vec<u8>)> {
            panic!("malformed framing must not reach application handler")
        }
        fn handle_failure(
            &mut self,
            context: metis_ipc::server::FailureContext,
            code: ErrorCode,
        ) -> Result<()> {
            self.failure = Some((context, code));
            Ok(())
        }
    }
    let arguments = [
        "/d",
        "/c",
        "echo 012345678901234567890123456789 & set /p INPUT=",
    ]
    .map(str::to_owned);
    let mut handler = Handler { failure: None };
    assert_eq!(
        run_session(&shell(), &arguments, &mut handler)
            .expect_err("invalid wire magic")
            .code,
        ErrorCode::MagicMismatch
    );
    assert_eq!(
        handler.failure,
        Some((
            metis_ipc::server::FailureContext::Receive,
            ErrorCode::MagicMismatch
        ))
    );
}
