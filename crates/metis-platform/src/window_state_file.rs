//! Saving and restoring a window's state in one host-chosen file.
//!
//! The file holds the text form of a [`WindowState`]. A save writes a fresh
//! sibling file and renames it over the old one, so a crash mid-save leaves
//! either the previous state or the new one, never a partial record. A load
//! reads at most [`MAX_WINDOW_STATE_BYTES`] and treats a missing file as "no
//! saved state" so a first launch uses the application's defaults.

use metis_core::window_state::{MAX_WINDOW_STATE_BYTES, WindowState};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// The file one window's state is saved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowStateFile {
    path: PathBuf,
    staging: PathBuf,
}

impl WindowStateFile {
    /// Binds the store to an absolute file path chosen by the host.
    ///
    /// # Errors
    /// Returns `InvalidInput` when the path is relative or has no file name.
    pub fn new(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let Some(name) = path.file_name().filter(|_| path.is_absolute()) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "window state file must be an absolute file path",
            ));
        };
        let mut staging_name = OsString::from(name);
        staging_name.push(".saving");
        let staging = path.with_file_name(staging_name);
        Ok(Self { path, staging })
    }

    /// The file the state is saved to.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads the saved state, or `None` when nothing has been saved yet.
    ///
    /// # Errors
    /// Returns the I/O error, or `InvalidData` when the file is larger than
    /// [`MAX_WINDOW_STATE_BYTES`] or is not a valid window state.
    pub fn load(&self) -> io::Result<Option<WindowState>> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut text = String::new();
        let limit = u64::try_from(MAX_WINDOW_STATE_BYTES).unwrap_or(u64::MAX);
        file.take(limit + 1)
            .read_to_string(&mut text)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        WindowState::decode(&text)
            .map(Some)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    /// Replaces the saved state, creating the parent directory if needed.
    ///
    /// # Errors
    /// Returns the I/O error from creating, writing, flushing or renaming the
    /// file.
    pub fn save(&self, state: &WindowState) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        match fs::remove_file(&self.staging) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
        // `create_new` refuses any file, including a link, left at the
        // staging path since the removal above.
        let mut staged = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.staging)?;
        let written = staged
            .write_all(state.encode().as_bytes())
            .and_then(|()| staged.sync_all());
        drop(staged);
        if let Err(error) = written.and_then(|()| fs::rename(&self.staging, &self.path)) {
            let _ = fs::remove_file(&self.staging);
            return Err(error);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
