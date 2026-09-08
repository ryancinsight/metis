//! Bounded session policy delegated to Moirai's process and blocking providers.
//!
//! Windows job containment controls child lifetimes, not token privileges,
//! filesystem/network access, or arbitrary synchronous handler execution.
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_ipc::{IpcHandler, IpcServer, StreamTransport};
use moirai_core::executor::TaskSpawner;
use moirai_executor::ExecutorBuilder;
use moirai_transport::process::{
    ManagedProcess, ProcessDropPolicy, ProcessError, ProcessSpec, ProcessStatus, ProcessSupervisor,
};
use std::{
    path::Path,
    process::ExitStatus,
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    time::{Duration, Instant},
};

/// Deadline for the demonstration child and its IPC session.
pub const SESSION_DEADLINE: Duration = Duration::from_secs(10);
/// Finite interaction budget for a visible desktop demonstration window.
pub const INTERACTIVE_SESSION_DEADLINE: Duration = Duration::from_mins(5);
/// Additional budget to confirm requested process termination.
const CLEANUP_DEADLINE: Duration = Duration::from_secs(1);
enum Completion {
    Closed,
    Failed,
}

/// Starts a contained presentation process and services its inherited pipes.
///
/// The Moirai blocking lane owns the child watchdog. Protocol failures request
/// immediate termination; expiration kills the job, closing descendant pipe
/// handles. Platforms without process-tree containment reject this session.
/// The handler must finish its synchronous operations; Rust cannot preempt it.
/// # Errors
/// Returns spawn, task admission, protocol, process, cleanup, or deadline errors.
pub fn run_session<H: IpcHandler>(
    binary: &Path,
    args: &[String],
    handler: &mut H,
) -> Result<ExitStatus> {
    run_session_with_deadline(binary, args, handler, SESSION_DEADLINE)
}

/// Starts a contained presentation process with an explicit finite deadline.
///
/// The deadline is a host policy input, not an unbounded wait: every child
/// session is terminated after it expires and its descendants are drained by
/// the same Moirai containment path as [`run_session`].
///
/// # Errors
/// Returns spawn, task admission, protocol, process, cleanup, or deadline errors.
pub fn run_session_with_deadline<H: IpcHandler>(
    binary: &Path,
    args: &[String],
    handler: &mut H,
    deadline: Duration,
) -> Result<ExitStatus> {
    let mut executor = ExecutorBuilder::new()
        .worker_threads(1)
        .async_threads(1)
        .build()
        .map_err(|error| task_error(&error))?;
    let session = (|| {
        let spec = ProcessSpec::new(binary)
            .args(args)
            .env_clear()
            .piped_stdio()
            .tree_containment();
        let mut child = ProcessSupervisor::new()
            .spawn(spec, ProcessDropPolicy::TerminateOnDrop)
            .map_err(process_error)?;
        let reader = child.take_stdout().ok_or_else(|| {
            MetisError::transport(
                ErrorCode::TransportBroken,
                "Process provider omitted requested stdout pipe",
            )
        })?;
        let writer = child.take_stdin().ok_or_else(|| {
            MetisError::transport(
                ErrorCode::TransportBroken,
                "Process provider omitted requested stdin pipe",
            )
        })?;
        let (finished, completion) = mpsc::sync_channel(1);
        let started = Instant::now();
        // Failed admission drops the closure and its owned kill-on-close job.
        let watchdog = executor
            .spawn_blocking(move || watch(child, &completion, started, deadline))
            .map_err(|error| task_error(&error))?;
        let processing: Result<()> = {
            let mut server = IpcServer::new(StreamTransport::new(reader, writer));
            (|| {
                while server.step(handler)? {}
                Ok(())
            })()
        };
        let notification = finished.send(if processing.is_ok() {
            Completion::Closed
        } else {
            Completion::Failed
        });
        let watched = watchdog
            .join()
            .ok_or_else(|| {
                MetisError::transport(
                    ErrorCode::TransportBroken,
                    "Watchdog task lost its result handle",
                )
            })?
            .map_err(|error| task_error(&error))?;
        // Timeout explains the read/write error caused by forced pipe closure.
        let status = watched?;
        processing?;
        notification.map_err(|_| {
            MetisError::transport(
                ErrorCode::TransportBroken,
                "Watchdog completion channel disconnected",
            )
        })?;
        if !status.exit_status().success() {
            return Err(MetisError::transport(
                ErrorCode::TransportBroken,
                format!("Frontend exited with {}", status.exit_status()),
            ));
        }
        Ok(status.exit_status())
    })();
    // Shutdown runs after every session outcome, including spawn/admission and
    // result-handle failure. Only this session's finite watchdog is submitted.
    let shutdown = executor.shutdown().map_err(|error| task_error(&error));
    session.and_then(|status| shutdown.map(|()| status))
}
fn watch(
    mut child: ManagedProcess,
    completion: &Receiver<Completion>,
    started: Instant,
    deadline: Duration,
) -> Result<ProcessStatus> {
    let remaining = deadline.saturating_sub(started.elapsed());
    match completion.recv_timeout(remaining) {
        Ok(Completion::Closed) => {
            if child
                .wait_timeout(deadline.saturating_sub(started.elapsed()))
                .map_err(process_error)?
                .is_some()
            {
                // A descendant can close IPC yet keep running after root exit.
                // Confirm job cleanup while retaining the root's exit status.
                return child
                    .terminate_timeout(CLEANUP_DEADLINE)
                    .map_err(process_error);
            }
        }
        Ok(Completion::Failed) | Err(RecvTimeoutError::Disconnected) => {
            return child
                .terminate_timeout(CLEANUP_DEADLINE)
                .map_err(process_error);
        }
        Err(RecvTimeoutError::Timeout) => {}
    }
    child
        .terminate_timeout(CLEANUP_DEADLINE)
        .map_err(process_error)?;
    Err(MetisError::transport(
        ErrorCode::Timeout,
        "Frontend session exceeded its deadline",
    ))
}
fn process_error(error: ProcessError) -> MetisError {
    MetisError::transport(
        if error == ProcessError::DeadlineExceeded {
            ErrorCode::Timeout
        } else {
            ErrorCode::TransportBroken
        },
        error.to_string(),
    )
}
fn task_error(error: &impl std::fmt::Display) -> MetisError {
    MetisError::transport(
        ErrorCode::TransportBroken,
        format!("Moirai watchdog failure: {error}"),
    )
}

#[cfg(all(test, windows))]
mod tests;
