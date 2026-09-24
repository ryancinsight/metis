//! Whole-file replacement that never leaves a partial file behind.

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

/// Replaces `path` with `bytes`, creating its parent directory if needed.
///
/// The bytes go to a fresh sibling that is flushed and renamed over `path`,
/// so a crash leaves either the old contents or the new ones. A file or link
/// left at the sibling path is removed first, and the sibling is created
/// exclusively so it is never written through a link.
pub(crate) fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut staging_name = OsString::from(name);
    staging_name.push(".saving");
    let staging = path.with_file_name(staging_name);
    match fs::remove_file(&staging) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    let mut staged = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging)?;
    let written = staged.write_all(bytes).and_then(|()| staged.sync_all());
    drop(staged);
    if let Err(error) = written.and_then(|()| fs::rename(&staging, path)) {
        let _ = fs::remove_file(&staging);
        return Err(error);
    }
    Ok(())
}
