//! Capability-witnessed direct process execution with bounded output.

use metis_core::capability::CapabilityScope;
use metis_core::host::VerifiedHostCapability;
use moirai_transport::process::{
    ManagedProcess, ProcessDropPolicy, ProcessError, ProcessOutcome, ProcessSpec, ProcessStatus,
    ProcessSupervisor,
};
use std::{
    ffi::{OsStr, OsString},
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    time::Duration,
};

/// Maximum number of arguments admitted by one scoped process request.
pub const MAX_SCOPED_PROCESS_ARGUMENTS: usize = 128;
/// Maximum encoded argument bytes admitted by one scoped process request.
pub const MAX_SCOPED_PROCESS_ARGUMENT_BYTES: usize = 64 * 1024;
/// Maximum bytes retained from either stdout or stderr.
pub const MAX_SCOPED_PROCESS_OUTPUT_BYTES: usize = 1024 * 1024;
/// Maximum number of explicitly configured child environment entries.
pub const MAX_SCOPED_PROCESS_ENVIRONMENT_ENTRIES: usize = 32;
/// Maximum encoded bytes across explicitly configured child environment entries.
pub const MAX_SCOPED_PROCESS_ENVIRONMENT_BYTES: usize = 16 * 1024;
/// Maximum process lifetime admitted by the provider.
pub const MAX_SCOPED_PROCESS_RUNTIME: Duration = Duration::from_secs(30);
const CLEANUP_DEADLINE: Duration = Duration::from_secs(1);

/// Process-tree policy selected by the trusted host configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessContainment {
    /// Supervise only the directly spawned process.
    DirectChild,
    /// Require the provider to contain descendants as one operating-system job.
    Tree,
}

/// Bounded process result with stderr content deliberately omitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedProcessOutput {
    status: ProcessStatus,
    stdout: Box<[u8]>,
    stderr_bytes: usize,
}

impl ScopedProcessOutput {
    /// Returns the operating-system completion status.
    #[must_use]
    pub const fn status(&self) -> ProcessStatus {
        self.status
    }

    /// Returns the captured stdout bytes.
    #[must_use]
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    /// Returns the number of stderr bytes drained without exposing their content.
    #[must_use]
    pub const fn stderr_bytes(&self) -> usize {
        self.stderr_bytes
    }

    /// Returns whether the process reported a successful exit status.
    #[must_use]
    pub const fn outcome(&self) -> ProcessOutcome {
        self.status.outcome
    }
}

/// Failure from validation, bounded I/O, or the Moirai process lifecycle.
#[derive(Debug)]
pub enum ScopedProcessError {
    /// The configured executable is not an absolute regular file.
    InvalidProgram,
    /// The request contains an argument outside the configured value allowlist.
    ArgumentDenied,
    /// The request exceeds the argument-count bound.
    TooManyArguments,
    /// The request exceeds the aggregate argument-byte bound.
    ArgumentsTooLarge,
    /// A configured child environment entry is malformed or exceeds its bound.
    InvalidEnvironment,
    /// The requested deadline is zero or exceeds the provider maximum.
    InvalidDeadline,
    /// A trusted host configuration or pipe operation failed.
    Io(io::Error),
    /// The process provider rejected or could not complete a lifecycle operation.
    Process(ProcessError),
    /// The process did not complete before its finite deadline.
    DeadlineExceeded,
    /// Cleanup could not confirm process termination.
    Cleanup(ProcessError),
    /// The requested stdio pipe was not returned by the process provider.
    MissingPipe,
    /// A bounded output stream exceeded its byte limit.
    OutputTooLarge,
    /// A bounded output reader terminated unexpectedly.
    ReaderPanicked,
}

impl std::fmt::Display for ScopedProcessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidProgram => "scoped process program must be an absolute regular file",
            Self::ArgumentDenied => "scoped process argument is outside the allowlist",
            Self::TooManyArguments => "scoped process argument count exceeds the provider bound",
            Self::ArgumentsTooLarge => "scoped process arguments exceed the provider byte bound",
            Self::InvalidEnvironment => "scoped process environment entry is invalid",
            Self::InvalidDeadline => "scoped process deadline is outside the provider bound",
            Self::Io(_) => "scoped process host I/O failed",
            Self::Process(_) => "scoped process lifecycle failed",
            Self::DeadlineExceeded => "scoped process deadline elapsed before completion",
            Self::Cleanup(_) => "scoped process cleanup did not complete",
            Self::MissingPipe => "scoped process output pipe was not created",
            Self::OutputTooLarge => "scoped process output exceeds the provider byte bound",
            Self::ReaderPanicked => "scoped process output reader terminated unexpectedly",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ScopedProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Process(error) | Self::Cleanup(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for ScopedProcessError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Native process provider with one trusted executable and argument values.
#[derive(Clone, PartialEq, Eq)]
pub struct ScopedProcessProvider {
    program: PathBuf,
    containment: ProcessContainment,
    allowed_arguments: Box<[OsString]>,
    environment: Box<[(OsString, OsString)]>,
}

impl std::fmt::Debug for ScopedProcessProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScopedProcessProvider")
            .field("containment", &self.containment)
            .field("allowed_argument_count", &self.allowed_arguments.len())
            .field("environment_entry_count", &self.environment.len())
            .finish_non_exhaustive()
    }
}

impl ScopedProcessProvider {
    /// Creates a provider for one trusted executable and argument-value allowlist.
    ///
    /// The executable path is host configuration, never browser input. Each
    /// request still requires a host capability witness and every argument must
    /// equal one of the configured values. The child receives an empty
    /// environment by default, direct stdio arguments (never a shell string),
    /// and an explicitly selected containment policy. Trusted host code may
    /// add a separately bounded environment allowlist with
    /// [`Self::with_environment`].
    ///
    /// # Errors
    /// Returns [`ScopedProcessError::InvalidProgram`] when `program` is not an
    /// absolute regular file, or [`ScopedProcessError::Io`] when its metadata
    /// cannot be inspected.
    pub fn new<A, I>(
        program: impl AsRef<Path>,
        containment: ProcessContainment,
        allowed_arguments: I,
    ) -> Result<Self, ScopedProcessError>
    where
        A: Into<OsString>,
        I: IntoIterator<Item = A>,
    {
        let program = program.as_ref();
        if !program.is_absolute() {
            return Err(ScopedProcessError::InvalidProgram);
        }
        let metadata = fs::symlink_metadata(program)?;
        if !metadata.file_type().is_file() {
            return Err(ScopedProcessError::InvalidProgram);
        }
        let mut checked_arguments = Vec::new();
        for argument in allowed_arguments {
            if checked_arguments.len() >= MAX_SCOPED_PROCESS_ARGUMENTS {
                return Err(ScopedProcessError::TooManyArguments);
            }
            checked_arguments.push(argument.into());
        }
        let allowed_arguments = checked_arguments.into_boxed_slice();
        if allowed_arguments
            .iter()
            .any(|argument| encoded_len(argument) > MAX_SCOPED_PROCESS_ARGUMENT_BYTES)
        {
            return Err(ScopedProcessError::ArgumentsTooLarge);
        }
        Ok(Self {
            program: program.to_path_buf(),
            containment,
            allowed_arguments,
            environment: Box::new([]),
        })
    }

    /// Adds a bounded, host-configured environment allowlist to the provider.
    ///
    /// The child still starts with an empty environment; only these entries
    /// are copied into it. Values are retained by the provider and never
    /// included in its [`Debug`] output.
    ///
    /// # Errors
    /// Returns [`ScopedProcessError::InvalidEnvironment`] when an entry is
    /// malformed, too numerous or exceeds the aggregate byte bound.
    pub fn with_environment<K, V, I>(mut self, entries: I) -> Result<Self, ScopedProcessError>
    where
        K: Into<OsString>,
        V: Into<OsString>,
        I: IntoIterator<Item = (K, V)>,
    {
        let mut environment = Vec::new();
        let mut total_bytes = 0usize;
        for (key, value) in entries {
            if environment.len() >= MAX_SCOPED_PROCESS_ENVIRONMENT_ENTRIES {
                return Err(ScopedProcessError::InvalidEnvironment);
            }
            let key = key.into();
            let value = value.into();
            if key.is_empty() || key.as_encoded_bytes().contains(&b'=') {
                return Err(ScopedProcessError::InvalidEnvironment);
            }
            total_bytes = total_bytes
                .checked_add(encoded_len(&key))
                .and_then(|bytes| bytes.checked_add(encoded_len(&value)))
                .ok_or(ScopedProcessError::InvalidEnvironment)?;
            if total_bytes > MAX_SCOPED_PROCESS_ENVIRONMENT_BYTES {
                return Err(ScopedProcessError::InvalidEnvironment);
            }
            environment.push((key, value));
        }
        self.environment = environment.into_boxed_slice();
        Ok(self)
    }

    /// Returns the trusted executable path.
    #[must_use]
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// Returns the configured process-tree policy.
    #[must_use]
    pub const fn containment(&self) -> ProcessContainment {
        self.containment
    }

    /// Returns the immutable argument-value allowlist.
    #[must_use]
    pub fn allowed_arguments(&self) -> &[OsString] {
        &self.allowed_arguments
    }

    /// Runs one allowlisted process with finite lifecycle and output bounds.
    ///
    /// Stderr is drained to prevent pipe backpressure but is never returned to
    /// the caller. This keeps diagnostics outside the browser-facing result;
    /// applications may record a redacted count while retaining the raw bytes
    /// inside the process boundary only long enough to enforce the bound.
    ///
    /// # Errors
    /// Returns a validation, lifecycle, cleanup, deadline or bounded-I/O error.
    pub fn run<A, I>(
        &self,
        _capability: &VerifiedHostCapability<{ CapabilityScope::RUN_PROCESS.0 }>,
        arguments: I,
        deadline: Duration,
    ) -> Result<ScopedProcessOutput, ScopedProcessError>
    where
        A: Into<OsString>,
        I: IntoIterator<Item = A>,
    {
        validate_deadline(deadline)?;
        let arguments = self.validate_arguments(arguments)?;
        let mut specification = ProcessSpec::new(self.program.clone())
            .args(arguments)
            .env_clear()
            .piped_stdio()
            .piped_stderr();
        for (key, value) in &self.environment {
            specification = specification.env(key.clone(), value.clone());
        }
        if self.containment == ProcessContainment::Tree {
            specification = specification.tree_containment();
        }
        let mut process = ProcessSupervisor::new()
            .spawn(specification, ProcessDropPolicy::TerminateOnDrop)
            .map_err(ScopedProcessError::Process)?;
        let stdout = process
            .take_stdout()
            .ok_or(ScopedProcessError::MissingPipe)?;
        let stderr = process
            .take_stderr()
            .ok_or(ScopedProcessError::MissingPipe)?;

        std::thread::scope(|scope| {
            let stdout_reader = scope.spawn(|| read_bounded(stdout));
            let stderr_reader = scope.spawn(|| read_bounded(stderr));
            let status = match process.wait_timeout(deadline) {
                Ok(Some(status)) => status,
                Ok(None) => {
                    terminate_after_deadline(&mut process)?;
                    return Err(ScopedProcessError::DeadlineExceeded);
                }
                Err(error) => {
                    terminate_after_error(&mut process)?;
                    return Err(ScopedProcessError::Process(error));
                }
            };
            let stdout = stdout_reader
                .join()
                .map_err(|_| ScopedProcessError::ReaderPanicked)??;
            let stderr = stderr_reader
                .join()
                .map_err(|_| ScopedProcessError::ReaderPanicked)??;
            Ok(ScopedProcessOutput {
                status,
                stderr_bytes: stderr.len(),
                stdout: stdout.into_boxed_slice(),
            })
        })
    }

    fn validate_arguments<A, I>(&self, arguments: I) -> Result<Vec<OsString>, ScopedProcessError>
    where
        A: Into<OsString>,
        I: IntoIterator<Item = A>,
    {
        let mut checked = Vec::new();
        let mut total_bytes = 0usize;
        for argument in arguments {
            if checked.len() >= MAX_SCOPED_PROCESS_ARGUMENTS {
                return Err(ScopedProcessError::TooManyArguments);
            }
            let argument = argument.into();
            if !self
                .allowed_arguments
                .iter()
                .any(|allowed| allowed == &argument)
            {
                return Err(ScopedProcessError::ArgumentDenied);
            }
            total_bytes = total_bytes
                .checked_add(encoded_len(&argument))
                .ok_or(ScopedProcessError::ArgumentsTooLarge)?;
            if total_bytes > MAX_SCOPED_PROCESS_ARGUMENT_BYTES {
                return Err(ScopedProcessError::ArgumentsTooLarge);
            }
            checked.push(argument);
        }
        Ok(checked)
    }
}

fn encoded_len(value: &OsStr) -> usize {
    value.as_encoded_bytes().len()
}

fn validate_deadline(deadline: Duration) -> Result<(), ScopedProcessError> {
    if deadline.is_zero() || deadline > MAX_SCOPED_PROCESS_RUNTIME {
        return Err(ScopedProcessError::InvalidDeadline);
    }
    Ok(())
}

fn terminate_after_deadline(process: &mut ManagedProcess) -> Result<(), ScopedProcessError> {
    process
        .terminate_timeout(CLEANUP_DEADLINE)
        .map(|_| ())
        .map_err(ScopedProcessError::Cleanup)
}

fn terminate_after_error(process: &mut ManagedProcess) -> Result<(), ScopedProcessError> {
    process
        .terminate_timeout(CLEANUP_DEADLINE)
        .map(|_| ())
        .map_err(ScopedProcessError::Cleanup)
}

fn read_bounded(reader: File) -> Result<Vec<u8>, ScopedProcessError> {
    let capacity = MAX_SCOPED_PROCESS_OUTPUT_BYTES
        .checked_add(1)
        .ok_or(ScopedProcessError::OutputTooLarge)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| ScopedProcessError::OutputTooLarge)?;
    reader
        .take(u64::try_from(capacity).map_err(|_| ScopedProcessError::OutputTooLarge)?)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_SCOPED_PROCESS_OUTPUT_BYTES {
        return Err(ScopedProcessError::OutputTooLarge);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
