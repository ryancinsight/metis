//! Real files are admitted without links and hashed with bounded streaming reads.
mod metadata;
use super::super::inventory::invalid;
use super::super::{Digest, FileIdentity, relative};
use std::{
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

#[cfg(test)]
mod tests;

impl FileIdentity {
    /// Streams a regular file after rejecting links and excessive size.
    ///
    /// # Errors
    /// Returns contextual filesystem, bound or concurrent-change errors.
    pub fn read(path: &Path, limit: u64) -> io::Result<Self> {
        unlinked_components(path)?;
        let metadata = metadata::read(path)?;
        if !metadata.is_file() || linked(&metadata) || metadata.len() > limit {
            return Err(invalid(format!(
                "input is linked, nonregular or oversized: {}",
                path.display()
            )));
        }
        let mut file = fs::File::open(path)?;
        let mut hash = moirai_crypto::Sha256::new();
        let mut bytes = 0_u64;
        let mut buffer = [0; 16 * 1024];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            bytes = bytes
                .checked_add(u64::try_from(count).map_err(|_| invalid("file length overflow"))?)
                .ok_or_else(|| invalid("file length overflow"))?;
            if bytes > limit || bytes > metadata.len() {
                return Err(invalid(format!(
                    "file grew during hashing: {}",
                    path.display()
                )));
            }
            hash.update(&buffer[..count]);
        }
        if bytes != metadata.len() {
            return Err(invalid(format!(
                "file changed during hashing: {}",
                path.display()
            )));
        }
        Ok(Self {
            bytes,
            digest: Digest::from_bytes(hash.finalize()),
        })
    }
    pub(super) fn verified_bytes(self, path: &Path, limit: u64) -> io::Result<Vec<u8>> {
        if Self::read(path, limit)? != self {
            return Err(invalid(format!("stale file identity: {}", path.display())));
        }
        let capacity =
            usize::try_from(self.bytes).map_err(|_| invalid("file allocation width overflow"))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(capacity)
            .map_err(|error| invalid(format!("asset allocation: {error}")))?;
        let mut file = fs::File::open(path)?;
        (&mut file).take(self.bytes).read_to_end(&mut bytes)?;
        let mut extra = [0];
        if file.read(&mut extra)? != 0 {
            return Err(invalid("file grew beyond its verified allocation bound"));
        }
        if Self::from_bytes(&bytes)? != self {
            return Err(invalid(format!(
                "file changed while retaining bytes: {}",
                path.display()
            )));
        }
        Ok(bytes)
    }
}

/// Resolves an existing regular file after rejecting each linked component.
///
/// # Errors
/// Rejects linked ancestors, absent files and nonregular files before resolving.
pub fn canonical_file(path: &Path) -> io::Result<PathBuf> {
    unlinked_components(path)?;
    if !metadata::read(path)?.is_file() {
        return Err(invalid("canonical input is not a regular file"));
    }
    path.canonicalize()
}
/// Resolves an existing directory after rejecting all linked components.
///
/// # Errors
/// Rejects absent, linked or non-directory components before canonicalization.
pub fn canonical_directory(path: &Path) -> io::Result<PathBuf> {
    unlinked_components(path)?;
    if !metadata::read(path)?.is_dir() {
        return Err(invalid("canonical input is not a directory"));
    }
    path.canonicalize()
}
fn unlinked_components(path: &Path) -> io::Result<()> {
    let mut component_path = PathBuf::new();
    let mut components = path.components().peekable();
    while let Some(component) = components.next() {
        component_path.push(component);
        // A namespace prefix followed by RootDir is only part of the root;
        // querying it alone admits no filesystem ancestor. Check the assembled
        // root on the next iteration. A prefix without RootDir retains its
        // native query: drive-relative and complete prefix-only forms differ.
        // Never normalize '..' first: the preceding linked node must reject.
        if matches!(component, Component::Prefix(_))
            && matches!(components.peek(), Some(Component::RootDir))
        {
            continue;
        }
        if linked(&metadata::read(&component_path)?) {
            return Err(invalid(format!(
                "linked input component: {}",
                component_path.display()
            )));
        }
    }
    Ok(())
}

/// Resolves a portable path beneath `root`, rejecting linked path components.
///
/// # Errors
/// Rejects ambiguous paths, linked components and nonregular final files.
pub fn source(root: &Path, relative_path: &str) -> io::Result<PathBuf> {
    relative(relative_path)?;
    let mut current = root.to_path_buf();
    let root_metadata = metadata::read(root)?;
    if !root_metadata.is_dir() || linked(&root_metadata) {
        return Err(invalid("source owner root is not an unlinked directory"));
    }
    for part in Path::new(relative_path).components() {
        current.push(part);
        if linked(&metadata::read(&current)?) {
            return Err(invalid("linked application input is not admitted"));
        }
    }
    if !current.is_file() {
        return Err(invalid("application input must be a regular file"));
    }
    Ok(current)
}
/// Identifies symbolic links or Windows reparse points.
#[must_use]
pub fn linked(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
