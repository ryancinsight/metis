//! Capability-witnessed hand-off of allowlisted URLs to the system opener.
//!
//! This is the counterpart of Tauri's opener plugin. A page never names a
//! program or a shell string: the trusted host fixes the launcher
//! ([`OpenLauncher`]) and an origin allowlist, each request needs the
//! [`CapabilityScope::OPEN_EXTERNAL`] witness, and the URL reaches the
//! launcher as one argument after strict validation ([`OpenTarget`]). The
//! launcher runs with an empty environment plus a fixed list of desktop
//! variables it needs to find the session, and it is not contained: the
//! browser it starts must outlive the request.

mod launcher;
mod target;

pub use launcher::OpenLauncher;
pub use target::{MAX_OPEN_URL_BYTES, OpenTarget};

use metis_core::capability::CapabilityScope;
use metis_core::host::{HostOrigin, VerifiedHostCapability};
use moirai_transport::process::{
    ProcessDropPolicy, ProcessError, ProcessOutcome, ProcessSpec, ProcessSupervisor,
};
use std::{fmt, time::Duration};

/// Maximum number of allowlisted origins.
pub const MAX_OPEN_ORIGINS: usize = 64;
/// Longest wait for the launcher to report.
pub const MAX_OPEN_DEADLINE: Duration = Duration::from_secs(10);

/// Why an open request was refused or failed.
#[derive(Debug)]
#[non_exhaustive]
pub enum OpenError {
    /// The URL is malformed, oversized or not an http(s) URL.
    InvalidUrl,
    /// The URL's origin is not allowlisted.
    OriginDenied,
    /// The allowlist is empty, oversized or names a non-http(s) origin.
    InvalidOrigins,
    /// The deadline is zero or exceeds [`MAX_OPEN_DEADLINE`].
    InvalidDeadline,
    /// The launcher could not be started.
    Process(ProcessError),
    /// The launcher exited unsuccessfully.
    LauncherFailed,
    /// The launcher did not report within the deadline; it keeps running and
    /// the request may still complete.
    DeadlineExceeded,
}

impl fmt::Display for OpenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidUrl => "open target is not a valid http(s) URL",
            Self::OriginDenied => "open target origin is not allowlisted",
            Self::InvalidOrigins => "opener origin allowlist is invalid",
            Self::InvalidDeadline => "open deadline is outside the provider bound",
            Self::Process(_) => "system opener could not be started",
            Self::LauncherFailed => "system opener reported failure",
            Self::DeadlineExceeded => "system opener did not report before its deadline",
        })
    }
}

impl std::error::Error for OpenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Process(error) => Some(error),
            _ => None,
        }
    }
}

/// Opens allowlisted http(s) URLs with a host-configured launcher.
#[derive(Clone, PartialEq, Eq)]
pub struct ScopedOpener {
    launcher: OpenLauncher,
    allowed_origins: Box<[HostOrigin]>,
}

impl fmt::Debug for ScopedOpener {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScopedOpener")
            .field("launcher", &self.launcher)
            .field("allowed_origin_count", &self.allowed_origins.len())
            .finish()
    }
}

impl ScopedOpener {
    /// Creates an opener for `launcher` admitting URLs on `origins`.
    ///
    /// # Errors
    /// Returns [`OpenError::InvalidOrigins`] when the allowlist is empty,
    /// exceeds [`MAX_OPEN_ORIGINS`], or names an origin that is not http(s).
    pub fn new<I, O>(launcher: OpenLauncher, origins: I) -> Result<Self, OpenError>
    where
        I: IntoIterator<Item = O>,
        O: AsRef<str>,
    {
        let mut allowed = Vec::new();
        for origin in origins {
            if allowed.len() == MAX_OPEN_ORIGINS {
                return Err(OpenError::InvalidOrigins);
            }
            let origin =
                HostOrigin::parse(origin.as_ref()).map_err(|_| OpenError::InvalidOrigins)?;
            if !target::is_web_origin(&origin) {
                return Err(OpenError::InvalidOrigins);
            }
            if !allowed.contains(&origin) {
                allowed.push(origin);
            }
        }
        if allowed.is_empty() {
            return Err(OpenError::InvalidOrigins);
        }
        Ok(Self {
            launcher,
            allowed_origins: allowed.into_boxed_slice(),
        })
    }

    /// The canonical allowlisted origins.
    #[must_use]
    pub fn allowed_origins(&self) -> &[HostOrigin] {
        &self.allowed_origins
    }

    /// Validates `url` against the allowlist without opening it.
    ///
    /// # Errors
    /// Returns [`OpenError::InvalidUrl`] or [`OpenError::OriginDenied`].
    pub fn check(&self, url: &str) -> Result<OpenTarget, OpenError> {
        let target = OpenTarget::parse(url)?;
        if self.allowed_origins.contains(target.origin()) {
            Ok(target)
        } else {
            Err(OpenError::OriginDenied)
        }
    }

    /// Hands `url` to the system opener and waits up to `deadline` for the
    /// launcher to report.
    ///
    /// # Errors
    /// Returns a validation error before anything runs, or a launcher error.
    pub fn open(
        &self,
        _capability: &VerifiedHostCapability<{ CapabilityScope::OPEN_EXTERNAL.0 }>,
        url: &str,
        deadline: Duration,
    ) -> Result<(), OpenError> {
        if deadline.is_zero() || deadline > MAX_OPEN_DEADLINE {
            return Err(OpenError::InvalidDeadline);
        }
        let target = self.check(url)?;
        let mut specification = ProcessSpec::new(self.launcher.program())
            .args(self.launcher.leading_arguments().iter().cloned())
            .arg(target.as_str())
            .env_clear();
        for (key, value) in launcher::session_environment() {
            specification = specification.env(key, value);
        }
        // Detached on drop: a launcher still running at the deadline may be
        // starting the browser, which the request must not kill.
        let mut process = ProcessSupervisor::new()
            .spawn(specification, ProcessDropPolicy::DetachOnDrop)
            .map_err(OpenError::Process)?;
        match process.wait_timeout(deadline).map_err(OpenError::Process)? {
            Some(status) if status.outcome == ProcessOutcome::Succeeded => Ok(()),
            Some(_) => Err(OpenError::LauncherFailed),
            None => Err(OpenError::DeadlineExceeded),
        }
    }
}

#[cfg(test)]
mod tests;
