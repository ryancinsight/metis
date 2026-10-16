//! Metadata failure context preserves native path units and the original error.
//!
//! The public IO error retains the OS error kind. Its owned context exposes the
//! original IO error through `Error::source`, including `raw_os_error`; the outer
//! contextual error is not itself an OS error. Context allocates only on failure.
use std::{
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
};

pub(super) fn read(path: &Path) -> io::Result<fs::Metadata> {
    fs::symlink_metadata(path).map_err(|source| {
        io::Error::new(
            source.kind(),
            Failure {
                path: path.to_path_buf(),
                source,
            },
        )
    })
}

struct Failure {
    path: PathBuf,
    source: io::Error,
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("symlink_metadata(")?;
        // Native OsStr Debug retains non-Unicode units; display() is lossy.
        fmt::Debug::fmt(self.path.as_os_str(), formatter)?;
        write!(formatter, "): {}", self.source)
    }
}

// Rust main's Result diagnostic uses Debug; retain the operation/path there.
impl fmt::Debug for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        // Condition 1: io::Error owns the native runtime-specific OS failure.
        Some(&self.source)
    }
}
