//! Bounded build-tool lifecycle uses Moirai, including its Windows job ownership.
use crate::Result;
use moirai_core::executor::TaskSpawner;
use moirai_executor::ExecutorBuilder;
use moirai_transport::process::{
    ManagedProcess, ProcessDropPolicy, ProcessSpec, ProcessSupervisor,
};
use std::{
    ffi::OsString,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

const CAPTURE_LIMIT: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy)]
pub(crate) enum Containment {
    Required,
    Uncontained,
}

pub(crate) fn tool(name: &str) -> Result<PathBuf> {
    let path = std::env::var_os("PATH").ok_or("PATH is absent")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| format!("required tool is unavailable on PATH: {name}").into())
}

pub(crate) fn run(program: &Path, args: &[OsString], timeout: Duration) -> Result<()> {
    let mut child = spawn(program, args, Containment::Required)?;
    let Some(status) = child.wait_timeout(timeout)? else {
        child.terminate_timeout(Duration::from_secs(1))?;
        return Err(format!(
            "tool exceeded its {} second deadline: {}",
            timeout.as_secs(),
            program.display()
        )
        .into());
    };
    if !status.exit_status().success() {
        return Err(format!(
            "tool failed: {} ({})",
            program.display(),
            status.exit_status()
        )
        .into());
    }
    Ok(())
}

pub(crate) fn spawn(
    program: &Path,
    args: &[OsString],
    containment: Containment,
) -> Result<ManagedProcess> {
    let spec = ProcessSpec::new(program).args(args);
    let spec = match containment {
        Containment::Required => spec.tree_containment(),
        Containment::Uncontained => spec,
    };
    Ok(ProcessSupervisor::new().spawn(spec, ProcessDropPolicy::TerminateOnDrop)?)
}

pub(crate) fn capture(program: &Path, args: &[OsString], timeout: Duration) -> Result<Vec<u8>> {
    let mut executor = ExecutorBuilder::new()
        .worker_threads(1)
        .async_threads(1)
        .build()?;
    let result = (|| {
        let mut child = ProcessSupervisor::new().spawn(
            ProcessSpec::new(program)
                .args(args)
                .piped_stdio()
                .tree_containment(),
            ProcessDropPolicy::TerminateOnDrop,
        )?;
        drop(child.take_stdin());
        let stdout = child
            .take_stdout()
            .ok_or("process provider omitted stdout")?;
        let reader = executor.spawn_blocking(move || {
            let mut bytes = Vec::new();
            stdout.take(CAPTURE_LIMIT + 1).read_to_end(&mut bytes)?;
            Ok::<_, std::io::Error>(bytes)
        })?;
        let status = child.wait_timeout(timeout)?;
        if status.is_none() {
            child.terminate_timeout(Duration::from_secs(1))?;
        }
        // Closing the job before joining closes inherited descendant pipe ends.
        drop(child);
        let bytes = reader
            .join()
            .ok_or("build output reader lost its result")???;
        let status = status.ok_or("build tool exceeded its finite deadline")?;
        if bytes.len() as u64 > CAPTURE_LIMIT {
            return Err("build output exceeds the 16 MiB capture budget".into());
        }
        if !status.exit_status().success() {
            return Err(format!("build tool failed: {}", status.exit_status()).into());
        }
        Ok(bytes)
    })();
    executor.shutdown()?;
    result
}
