//! Safe installation and removal of Linux USTAR application packages.

use super::{desktop_icon, desktop_word};
use crate::{Result, manifest};
use moirai_crypto::Sha256;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

mod archive;
use archive::{ArchiveEntry, read_archive, safe_relative};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InstallationRecord {
    schema: u32,
    application_id: String,
    prefix: String,
    files: Vec<InstalledFile>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InstalledFile {
    path: String,
    bytes: u64,
    sha256: String,
}

/// Installs a Linux package below an absolute user or system prefix.
pub(crate) fn run(archive: &Path, prefix: &Path) -> Result<()> {
    if !cfg!(target_os = "linux") {
        return Err("Linux package installation requires a Linux host".into());
    }
    let prefix = canonical_prefix(prefix)?;
    install_archive(archive, &prefix)
}

/// Removes one installation while preserving files changed by the user.
pub(crate) fn uninstall(application_id: &str, prefix: &Path) -> Result<()> {
    if !cfg!(target_os = "linux") {
        return Err("Linux package removal requires a Linux host".into());
    }
    let prefix = canonical_prefix(prefix)?;
    remove_installation(application_id, &prefix)
}

#[cfg(any(not(windows), test))]
pub(crate) fn install_archive(archive: &Path, prefix: &Path) -> Result<()> {
    let prefix = canonical_prefix(prefix)?;
    let archive = canonical_file(archive)?;
    let entries = read_archive(&archive)?;
    let (application_id, desktop_index) = application_identity(&entries)?;
    let marker = marker_path(&prefix, &application_id)?;
    if marker.try_exists()? {
        return Err("application is already installed under this prefix".into());
    }

    let mut planned = Vec::with_capacity(entries.len());
    let mut destinations = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        let relative = package_relative(&entry.path)?;
        let relative = if index == desktop_index {
            rewrite_desktop_relative(&entry.path, relative, &application_id)?
        } else {
            relative.to_owned()
        };
        if !destinations.insert(relative.clone()) {
            return Err("package contains duplicate installation paths".into());
        }
        let target = prefix.join(&relative);
        if target.try_exists()? || fs::symlink_metadata(&target).is_ok() {
            return Err(format!("installation path already exists: {relative}").into());
        }
        planned.push((relative, target));
    }

    let mut created_files = Vec::with_capacity(planned.len() + 1);
    let mut created_dirs = Vec::new();
    let result = (|| {
        let mut files = Vec::with_capacity(planned.len());
        for ((relative, target), entry) in planned.iter().zip(entries.iter()) {
            ensure_parent(&prefix, target, &mut created_dirs)?;
            let bytes = if entry.path == entries[desktop_index].path {
                rewrite_desktop(&entry.bytes, &application_id, &prefix, &entries)?
            } else {
                entry.bytes.clone()
            };
            write_file(target, &bytes, entry.mode)?;
            created_files.push(target.clone());
            files.push(InstalledFile {
                path: relative.clone(),
                bytes: u64::try_from(bytes.len()).map_err(|_| "installed file is too large")?,
                sha256: digest(&bytes),
            });
        }
        ensure_parent(&prefix, &marker, &mut created_dirs)?;
        let record = InstallationRecord {
            schema: 1,
            application_id: application_id.clone(),
            prefix: prefix.to_string_lossy().into_owned(),
            files,
        };
        let encoded = serde_json::to_vec_pretty(&record)?;
        write_file(&marker, &encoded, 0o600)?;
        created_files.push(marker);
        Ok::<(), Box<dyn std::error::Error>>(())
    })();
    if let Err(error) = result {
        if let Err(rollback) = rollback(&created_files, &created_dirs) {
            return Err(
                format!("installation failed: {error}; rollback failed: {rollback}").into(),
            );
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(any(not(windows), test))]
fn remove_installation(application_id: &str, prefix: &Path) -> Result<()> {
    validate_application_id(application_id)?;
    let marker = marker_path(prefix, application_id)?;
    let bytes = fs::read(&marker)?;
    let record: InstallationRecord = serde_json::from_slice(&bytes)?;
    if record.schema != 1
        || record.application_id != application_id
        || record.prefix != prefix.to_string_lossy()
    {
        return Err("installation record does not match the requested prefix or identity".into());
    }
    let mut changed = Vec::new();
    for file in &record.files {
        let relative = safe_relative(&file.path)?;
        let target = prefix.join(relative);
        if target.try_exists()? {
            let metadata = fs::symlink_metadata(&target)?;
            let modified = !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.len() != file.bytes
                || digest_file(&target)? != file.sha256;
            if modified {
                changed.push(file.path.clone());
            }
        }
    }
    if !changed.is_empty() {
        return Err(format!(
            "refusing to remove user-modified installation files: {}",
            changed.join(", ")
        )
        .into());
    }
    for file in &record.files {
        let target = prefix.join(safe_relative(&file.path)?);
        if target.try_exists()? {
            fs::remove_file(&target)?;
        }
    }
    fs::remove_file(&marker)?;
    cleanup_empty_dirs(prefix, application_id)?;
    Ok(())
}

#[cfg(any(not(windows), test))]
fn application_identity(entries: &[ArchiveEntry]) -> Result<(String, usize)> {
    let mut identity = None;
    for (index, entry) in entries.iter().enumerate() {
        let Some(name) = entry.path.strip_prefix("usr/share/applications/") else {
            continue;
        };
        let Some(id) = name.strip_suffix(".desktop") else {
            return Err("Linux archive desktop entry has an invalid name".into());
        };
        if id.is_empty() || id.contains('/') {
            return Err("Linux archive desktop identity is invalid".into());
        }
        validate_application_id(id)?;
        if identity.replace((id.to_owned(), index)).is_some() {
            return Err("Linux archive contains multiple desktop entries".into());
        }
    }
    identity.ok_or_else(|| "Linux archive has no desktop entry".into())
}

#[cfg(any(not(windows), test))]
fn rewrite_desktop(
    bytes: &[u8],
    application_id: &str,
    prefix: &Path,
    entries: &[ArchiveEntry],
) -> Result<Vec<u8>> {
    let text = std::str::from_utf8(bytes).map_err(|_| "desktop entry is not UTF-8")?;
    let mut output = String::new();
    let mut executable = None;
    let mut icon = None;
    for line in text.lines() {
        let line = if let Some(value) = line.strip_prefix("Exec=") {
            let (program, suffix) = value
                .find(char::is_whitespace)
                .map_or((value, ""), |index| value.split_at(index));
            let relative = program
                .strip_prefix("/usr/bin/")
                .ok_or("desktop entry executable is not in the package bin directory")?;
            manifest::relative(relative)?;
            let package_path = format!("usr/bin/{relative}");
            if !entries.iter().any(|entry| entry.path == package_path) {
                return Err("desktop entry executable is absent from the package".into());
            }
            executable = Some(relative.to_owned());
            let path = prefix.join("bin").join(relative);
            format!("Exec={}{}", desktop_word(&path.to_string_lossy()), suffix)
        } else if let Some(value) = line.strip_prefix("Icon=") {
            let relative = value
                .strip_prefix(&format!("/usr/share/{application_id}/"))
                .ok_or("desktop icon is outside the package resource directory")?;
            manifest::relative(relative)?;
            let package_path = format!("usr/share/{application_id}/{relative}");
            if !entries.iter().any(|entry| entry.path == package_path) {
                return Err("desktop icon is absent from the package".into());
            }
            icon = Some(relative.to_owned());
            let path = prefix.join("share").join(application_id).join(relative);
            format!("Icon={}", desktop_icon(&path.to_string_lossy()))
        } else {
            line.to_owned()
        };
        output.push_str(&line);
        output.push('\n');
    }
    if executable.is_none() {
        return Err("desktop entry has no executable".into());
    }
    if icon.is_some() && !text.lines().any(|line| line.starts_with("Icon=")) {
        return Err("desktop icon state is inconsistent".into());
    }
    Ok(output.into_bytes())
}

#[cfg(any(not(windows), test))]
fn rewrite_desktop_relative(path: &str, relative: &str, application_id: &str) -> Result<String> {
    let expected = format!("usr/share/applications/{application_id}.desktop");
    if path != expected || relative != format!("share/applications/{application_id}.desktop") {
        return Err("Linux archive desktop path is inconsistent with its identity".into());
    }
    Ok(relative.to_owned())
}

#[cfg(any(not(windows), test))]
fn package_relative(path: &str) -> Result<&str> {
    let relative = path
        .strip_prefix("usr/")
        .ok_or("Linux archive entry is outside the usr prefix")?;
    safe_relative(relative)
}

#[cfg(any(not(windows), test))]
fn canonical_prefix(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err("installation prefix must be absolute".into());
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("installation prefix must be an existing directory".into());
    }
    Ok(path.canonicalize()?)
}

#[cfg(any(not(windows), test))]
fn canonical_file(path: &Path) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("Linux archive must be a regular file".into());
    }
    Ok(path.canonicalize()?)
}

#[cfg(any(not(windows), test))]
fn marker_path(prefix: &Path, application_id: &str) -> Result<PathBuf> {
    validate_application_id(application_id)?;
    Ok(prefix
        .join("share")
        .join("metis")
        .join(application_id)
        .join("installation.json"))
}

#[cfg(any(not(windows), test))]
fn validate_application_id(value: &str) -> Result<()> {
    if value.contains('/') || value.contains('\\') {
        return Err("installation identity must be a single path component".into());
    }
    manifest::relative(value)
}

#[cfg(any(not(windows), test))]
fn ensure_parent(prefix: &Path, target: &Path, created: &mut Vec<PathBuf>) -> Result<()> {
    let relative = target
        .strip_prefix(prefix)
        .map_err(|_| "installation target escaped its prefix")?;
    let mut current = prefix.to_path_buf();
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    for component in parent.components() {
        let Component::Normal(part) = component else {
            return Err("installation parent is not relative".into());
        };
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => return Err("installation parent contains a non-directory or symlink".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current)?;
                created.push(current.clone());
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

#[cfg(any(not(windows), test))]
fn write_file(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    #[cfg(not(unix))]
    let _ = mode;
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o777))?;
    }
    Ok(())
}

#[cfg(any(not(windows), test))]
fn rollback(files: &[PathBuf], dirs: &[PathBuf]) -> Result<()> {
    for file in files.iter().rev() {
        match fs::remove_file(file) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    for directory in dirs.iter().rev() {
        match fs::remove_dir(directory) {
            Ok(()) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
                ) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

#[cfg(any(not(windows), test))]
fn cleanup_empty_dirs(prefix: &Path, application_id: &str) -> Result<()> {
    let roots = [
        prefix.join("share").join("metis").join(application_id),
        prefix.join("share").join("metis"),
        prefix.join("share").join("applications"),
        prefix.join("share").join(application_id),
        prefix.join("share"),
        prefix.join("bin"),
    ];
    for directory in roots {
        match fs::remove_dir(directory) {
            Ok(()) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
                ) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

#[cfg(any(not(windows), test))]
fn digest_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hex(&hash.finalize()))
}

#[cfg(any(not(windows), test))]
fn digest(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(bytes);
    hex(&hash.finalize())
}

#[cfg(any(not(windows), test))]
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").expect("invariant: String formatting cannot fail");
    }
    output
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod tests;
