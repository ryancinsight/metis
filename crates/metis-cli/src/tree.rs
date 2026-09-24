//! Bounded enumeration of the regular files below a directory.
use crate::{Result, manifest};
use std::{
    collections::VecDeque,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

const FILE_LIMIT: usize = 4096;
const DEPTH_LIMIT: usize = 64;

/// Returns every regular file below `root` in sorted order, skipping the
/// directories `ignored` names. Links and special files are rejected rather
/// than followed, so the walk cannot leave `root`.
pub(crate) fn regular_files(root: &Path, ignored: impl Fn(&OsStr) -> bool) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending = VecDeque::from([(root.to_path_buf(), 0_usize)]);
    while let Some((directory, depth)) = pending.pop_front() {
        if depth > DEPTH_LIMIT {
            return Err(format!("{} exceeds the 64-level depth budget", root.display()).into());
        }
        let mut entries = fs::read_dir(&directory)?.collect::<std::result::Result<Vec<_>, _>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if manifest::linked(&metadata) {
                return Err(format!("{} is a link", path.display()).into());
            }
            if metadata.is_dir() {
                if !ignored(&entry.file_name()) {
                    pending.push_back((path, depth + 1));
                }
            } else if metadata.is_file() {
                files.push(path);
                if files.len() > FILE_LIMIT {
                    return Err(format!("{} exceeds the 4096-file budget", root.display()).into());
                }
            } else {
                return Err(format!("{} is not a regular file", path.display()).into());
            }
        }
    }
    files.sort();
    Ok(files)
}

/// `path` below `root` as a `/`-separated name, the form manifests and URLs use.
pub(crate) fn relative_name(root: &Path, path: &Path) -> Result<String> {
    let parts = path
        .strip_prefix(root)
        .map_err(|_| format!("{} is outside {}", path.display(), root.display()))?
        .components()
        .map(|part| {
            part.as_os_str()
                .to_str()
                .ok_or_else(|| format!("{} is not Unicode", path.display()))
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(parts.join("/"))
}
