//! One provider-owned bounded subprocess capture for preparation and tool identity.
mod failure;
use super::super::inventory::invalid;
use moirai_core::executor::{ExecutorConfig, TaskSpawner};
use moirai_executor::HybridExecutor;
use moirai_transport::process::{ProcessDropPolicy, ProcessSpec, ProcessSupervisor};
use std::{
    ffi::OsString,
    io::{self, Read},
    path::Path,
    time::Duration,
};

#[cfg(test)]
mod tests;

/// Existing Cargo/tool stdout admission bound, independent of manifest size.
pub const TOOL_OUTPUT_LIMIT: u64 = 16 * 1024 * 1024;
/// Existing bounded identity-query deadline.
pub const TOOL_IDENTITY_DEADLINE: Duration = Duration::from_secs(30);

/// Captures an actual tool through the provider's owned child, pipes and tree.
///
/// # Errors
/// Preserves OS/process/executor failures, timeout, unsuccessful exit and output
/// overflow. A failed tool does not produce an accepted partial stdout value.
/// Nonzero exits retain the exact program, native status and bounded stdout.
/// Valid UTF-8 renders verbatim; other bytes use tagged, reversible ASCII
/// escaping, including escape of literal backslashes. No replacement character
/// or truncated prefix substitutes for the collected bytes. Stderr remains
/// inherited. A simultaneous shutdown failure retains the primary error.
pub fn capture_command(
    program: &Path,
    args: &[OsString],
    timeout: Duration,
    environment: &[(&str, OsString)],
    working_directory: Option<&Path>,
) -> io::Result<Vec<u8>> {
    let started = std::time::Instant::now();
    let _span = tracing::info_span!("capture_build_tool", program = %program.display(), argument_count = args.len(), deadline_millis = timeout.as_millis()).entered();
    let mut executor = HybridExecutor::new(ExecutorConfig {
        worker_threads: 1,
        async_threads: 1,
        ..ExecutorConfig::default()
    })
    .map_err(io::Error::other)?;
    let result = (|| {
        let spec = environment.iter().fold(
            ProcessSpec::new(program).args(args),
            |spec, (key, value)| spec.env(*key, value),
        );
        let spec = match working_directory {
            Some(directory) => spec.working_directory(directory),
            None => spec,
        };
        let mut child = ProcessSupervisor::new()
            .spawn(
                spec.piped_stdio().tree_containment(),
                ProcessDropPolicy::TerminateOnDrop,
            )
            .map_err(io::Error::other)?;
        drop(child.take_stdin());
        let stdout = child
            .take_stdout()
            .ok_or_else(|| invalid("process provider omitted stdout"))?;
        let span = tracing::Span::current();
        let dispatcher = tracing::dispatcher::get_default(Clone::clone);
        let reader = executor
            .spawn_blocking(move || {
                tracing::dispatcher::with_default(&dispatcher, || {
                    span.in_scope(|| {
                        let _read = tracing::info_span!("read_tool_output").entered();
                        let mut bytes = Vec::new();
                        stdout.take(TOOL_OUTPUT_LIMIT + 1).read_to_end(&mut bytes)?;
                        Ok::<_, io::Error>(bytes)
                    })
                })
            })
            .map_err(io::Error::other)?;
        let status = child.wait_timeout(timeout).map_err(io::Error::other)?;
        if status.is_none() {
            child
                .terminate_timeout(Duration::from_secs(1))
                .map_err(io::Error::other)?;
        }
        drop(child);
        let bytes = reader
            .join()
            .ok_or_else(|| invalid("build output reader lost its result"))?
            .map_err(io::Error::other)??;
        let status = status.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                "build tool exceeded its finite deadline",
            )
        })?;
        if u64::try_from(bytes.len()).map_err(|_| invalid("tool output length overflow"))?
            > TOOL_OUTPUT_LIMIT
        {
            return Err(invalid("build output exceeds the 16 MiB capture budget"));
        }
        if !status.exit_status().success() {
            return Err(io::Error::other(failure::ToolFailure {
                program: program.to_path_buf(),
                status: status.exit_status(),
                stdout: bytes,
            }));
        }
        Ok(bytes)
    })();
    let cleanup = executor.shutdown().map_err(io::Error::other);
    tracing::info!(
        elapsed_millis = started.elapsed().as_millis(),
        successful = result.is_ok(),
        "tool capture completed"
    );
    match (result, cleanup) {
        (Ok(bytes), Ok(())) => Ok(bytes),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(primary), Err(cleanup)) => Err(io::Error::other(failure::CleanupFailure {
            primary,
            cleanup,
        })),
    }
}
