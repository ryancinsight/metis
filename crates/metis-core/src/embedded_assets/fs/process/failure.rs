//! Cold diagnostic ownership retains native identity and every admitted byte.
//!
//! UTF-8 is rendered verbatim. Other stdout uses `escape_ascii`: printable
//! ASCII retains its value, control/non-ASCII bytes use escapes, and literal
//! escape characters are escaped. This encoding is injective over byte strings.
//! Both forms retain the original byte vector and report its exact length.
use std::{error::Error, fmt, io, path::PathBuf, process::ExitStatus};

pub(super) struct ToolFailure {
    pub(super) program: PathBuf,
    pub(super) status: ExitStatus,
    pub(super) stdout: Vec<u8>,
}

impl fmt::Display for ToolFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("build tool failed: ")?;
        // Native OsStr Debug escapes non-Unicode path units without loss;
        // Path::display would substitute replacement characters.
        fmt::Debug::fmt(self.program.as_os_str(), formatter)?;
        writeln!(formatter, " ({})", self.status)?;
        if let Ok(text) = std::str::from_utf8(&self.stdout) {
            writeln!(formatter, "stdout ({} bytes; UTF-8):", self.stdout.len())?;
            formatter.write_str(text)
        } else {
            writeln!(
                formatter,
                "stdout ({} bytes; escaped bytes):",
                self.stdout.len()
            )?;
            write!(formatter, "{}", self.stdout.escape_ascii())
        }
    }
}

// Rust main reports Result errors through Debug. The domain diagnostic uses
// its lossless rendering there too; structural Vec Debug would hide compiler
// messages in byte-number arrays. Both traits retain one formatting owner.
impl fmt::Debug for ToolFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl Error for ToolFailure {}

/// Preserve the primary owned diagnostic when executor cleanup also fails.
pub(super) struct CleanupFailure {
    pub(super) primary: io::Error,
    pub(super) cleanup: io::Error,
}
impl fmt::Display for CleanupFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}; executor shutdown also failed: {}",
            self.primary, self.cleanup
        )
    }
}
// The same cold reporting contract applies to combined failures.
impl fmt::Debug for CleanupFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl Error for CleanupFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        // Condition 1: io::Error owns the provider's runtime-specific error.
        Some(&self.primary)
    }
}
