//! Launching an application when the user logs in.
//!
//! This is the counterpart of Tauri's autostart plugin. An [`Autostart`]
//! names one login item: an absolute program path and its arguments. On
//! Linux and other freedesktop systems it is an XDG autostart desktop entry,
//! on macOS a per-user launch agent, and on Windows a value in the user's
//! `Run` key. Each form is per-user, so enabling needs no elevation, and the
//! command is quoted by `metis_core::command_line` for the parser that reads
//! it back.

#[cfg(any(not(windows), test))]
mod entry;

use std::io;
use std::path::Path;
#[cfg(not(windows))]
use std::path::PathBuf;

/// Maximum arguments a login item passes.
pub const MAX_AUTOSTART_ARGUMENTS: usize = 32;
/// Maximum bytes in one login-item argument.
pub const MAX_AUTOSTART_ARGUMENT_BYTES: usize = 1_024;
const MAX_NAME_BYTES: usize = 64;

/// One application's login item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Autostart {
    name: String,
    program: String,
    arguments: Vec<String>,
    #[cfg(not(windows))]
    entry: PathBuf,
}

impl Autostart {
    /// Describes the login item `name` that starts `program` with
    /// `arguments`.
    ///
    /// `name` is 1 to 64 lowercase letters, digits, `.` and `-`, such as a
    /// reverse-DNS application identifier.
    ///
    /// # Errors
    /// Returns `InvalidInput` for an invalid name, a relative or non-UTF-8
    /// program path, text with control characters or arguments over their
    /// bounds, and `NotFound` when the per-user location cannot be resolved.
    pub fn new<I, S>(name: &str, program: impl AsRef<Path>, arguments: I) -> io::Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        #[cfg(not(windows))]
        let base = entry::user_directory()?;
        Self::build(
            name,
            program.as_ref(),
            arguments,
            #[cfg(not(windows))]
            &base,
        )
    }

    fn build<I, S>(
        name: &str,
        program: &Path,
        arguments: I,
        #[cfg(not(windows))] base: &Path,
    ) -> io::Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let name_valid = !name.is_empty()
            && name.len() <= MAX_NAME_BYTES
            && name.as_bytes()[0].is_ascii_alphanumeric()
            && name.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
            });
        let program = program
            .to_str()
            .filter(|text| program.is_absolute() && !text.chars().any(char::is_control))
            .map(str::to_owned);
        let mut owned = Vec::new();
        for argument in arguments {
            let argument = argument.into();
            if owned.len() >= MAX_AUTOSTART_ARGUMENTS
                || argument.len() > MAX_AUTOSTART_ARGUMENT_BYTES
                || argument.chars().any(char::is_control)
            {
                return Err(invalid("login item arguments exceed their bounds"));
            }
            owned.push(argument);
        }
        let (true, Some(program)) = (name_valid, program) else {
            return Err(invalid("login item name or program path is invalid"));
        };
        Ok(Self {
            #[cfg(not(windows))]
            entry: entry::path(base, name),
            name: name.to_owned(),
            program,
            arguments: owned,
        })
    }

    /// Registers the login item, replacing an earlier registration.
    ///
    /// # Errors
    /// Returns the I/O or registry error.
    pub fn enable(&self) -> io::Result<()> {
        #[cfg(windows)]
        return moirai_pal::windows::startup::set_startup_command(&self.name, &self.command());
        #[cfg(not(windows))]
        crate::atomic_file::replace(&self.entry, self.document().as_bytes())
    }

    /// Removes the login item; returns whether one was registered.
    ///
    /// # Errors
    /// Returns the I/O or registry error.
    pub fn disable(&self) -> io::Result<bool> {
        #[cfg(windows)]
        return moirai_pal::windows::startup::remove_startup_command(&self.name);
        #[cfg(not(windows))]
        match std::fs::remove_file(&self.entry) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Whether this exact login item is registered; an entry under the same
    /// name with a different command reports `false`.
    ///
    /// # Errors
    /// Returns the I/O or registry error.
    pub fn is_enabled(&self) -> io::Result<bool> {
        #[cfg(windows)]
        return moirai_pal::windows::startup::startup_command(&self.name)
            .map(|command| command.as_deref() == Some(self.command().as_str()));
        #[cfg(not(windows))]
        match std::fs::read(&self.entry) {
            Ok(bytes) => Ok(bytes == self.document().as_bytes()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    #[cfg(windows)]
    fn command(&self) -> String {
        metis_core::command_line::windows_command_line(&self.program, &self.arguments)
    }

    #[cfg(not(windows))]
    fn document(&self) -> String {
        entry::document(&self.name, &self.program, &self.arguments)
    }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests;
