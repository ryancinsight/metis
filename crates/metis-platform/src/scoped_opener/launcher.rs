//! The host program that opens a URL in the user's default handler.

use std::{
    ffi::OsString,
    fmt,
    path::{Path, PathBuf},
};

/// A trusted launcher program and the fixed arguments before the URL.
#[derive(Clone, PartialEq, Eq)]
pub struct OpenLauncher {
    program: PathBuf,
    leading_arguments: Box<[OsString]>,
}

impl fmt::Debug for OpenLauncher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenLauncher")
            .field("program", &self.program)
            .field("leading_argument_count", &self.leading_arguments.len())
            .finish()
    }
}

impl OpenLauncher {
    /// A host-chosen launcher: an absolute path to a regular file, and the
    /// arguments that precede the URL.
    ///
    /// # Errors
    /// Returns [`std::io::ErrorKind::InvalidInput`] for a relative path, or
    /// the metadata error when the program is not a readable regular file.
    pub fn new<I, A>(program: impl Into<PathBuf>, leading_arguments: I) -> std::io::Result<Self>
    where
        I: IntoIterator<Item = A>,
        A: Into<OsString>,
    {
        let program = program.into();
        if !program.is_absolute() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "opener launcher must be an absolute path",
            ));
        }
        if !std::fs::metadata(&program)?.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "opener launcher must be a regular file",
            ));
        }
        Ok(Self {
            program,
            leading_arguments: leading_arguments.into_iter().map(Into::into).collect(),
        })
    }

    /// The platform's standard opener at its standard location:
    /// `xdg-open` on Linux and the BSDs, `open` on macOS, and the URL
    /// protocol handler through `rundll32` on Windows. `None` when it is not
    /// installed there; nothing is looked up on `PATH`.
    #[must_use]
    pub fn platform_default() -> Option<Self> {
        platform_candidates()
            .into_iter()
            .find_map(|(program, arguments)| Self::new(program, arguments.iter().copied()).ok())
    }

    /// The launcher program.
    #[must_use]
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// Arguments passed before the URL.
    #[must_use]
    pub fn leading_arguments(&self) -> &[OsString] {
        &self.leading_arguments
    }
}

#[cfg(windows)]
fn platform_candidates() -> Vec<(PathBuf, &'static [&'static str])> {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    vec![(
        PathBuf::from(root).join("System32").join("rundll32.exe"),
        &["url.dll,FileProtocolHandler"],
    )]
}

#[cfg(target_vendor = "apple")]
fn platform_candidates() -> Vec<(PathBuf, &'static [&'static str])> {
    vec![(PathBuf::from("/usr/bin/open"), &[])]
}

#[cfg(not(any(windows, target_vendor = "apple")))]
fn platform_candidates() -> Vec<(PathBuf, &'static [&'static str])> {
    [
        "/usr/bin/xdg-open",
        "/usr/local/bin/xdg-open",
        "/bin/xdg-open",
    ]
    .into_iter()
    .map(|path| (PathBuf::from(path), &[][..]))
    .collect()
}

/// Variables a launcher needs to reach the user's desktop session. The
/// launcher starts from an empty environment and receives only these.
#[cfg(windows)]
const SESSION_VARIABLES: &[&str] = &["SystemRoot", "windir", "USERPROFILE", "TEMP", "TMP"];
#[cfg(not(windows))]
const SESSION_VARIABLES: &[&str] = &[
    "PATH",
    "HOME",
    "LANG",
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "XDG_RUNTIME_DIR",
    "XDG_CURRENT_DESKTOP",
    "XDG_DATA_DIRS",
    "XDG_CONFIG_DIRS",
    "DBUS_SESSION_BUS_ADDRESS",
];

/// The allowlisted session variables present in this process.
pub(super) fn session_environment() -> impl Iterator<Item = (&'static str, OsString)> {
    SESSION_VARIABLES
        .iter()
        .filter_map(|name| std::env::var_os(name).map(|value| (*name, value)))
}
