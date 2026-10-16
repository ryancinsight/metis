//! Canonical directory membership detects additions, removals and empty directories.
use super::super::{
    Digest, DirectoryPolicy, FILE_LIMIT, PAYLOAD_LIMIT, inventory::invalid, relative,
};
use super::{FileIdentity, canonical_directory};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::Path,
};

/// Complete bounded membership and file identities for a declared input directory.
#[derive(Debug)]
pub struct DirectorySnapshot {
    digest: Digest,
    files: Vec<(String, FileIdentity)>,
}
impl DirectorySnapshot {
    /// Reads every member under a declared owner-relative scope.
    ///
    /// `None` selects the owner root. The `CargoPackage` policy excludes package
    /// infrastructure directories `.git` and `target`; `Declared` includes all
    /// members, including directories with either name.
    /// The digest includes empty directories and exact file lengths/hashes.
    ///
    /// # Errors
    /// Rejects links, special files, ambiguous paths, count/byte overflows.
    pub fn read(root: &Path, scope: Option<&str>, policy: DirectoryPolicy) -> io::Result<Self> {
        if canonical_directory(root)? != root {
            return Err(invalid("directory owner root is not canonical"));
        }
        let directory = match scope {
            Some(scope) => {
                relative(scope)?;
                root.join(scope)
            }
            None => root.to_path_buf(),
        };
        if canonical_directory(&directory)? != directory {
            return Err(invalid("directory scope is not canonical"));
        }
        let mut pending = vec![directory.clone()];
        let mut entries = BTreeMap::new();
        let mut files = Vec::new();
        let mut folded = BTreeSet::new();
        let mut total = 0_u64;
        while let Some(current) = pending.pop() {
            for entry in fs::read_dir(&current)? {
                let entry = entry?;
                let path = entry.path();
                let metadata = fs::symlink_metadata(&path)?;
                if super::linked(&metadata) {
                    return Err(invalid("declared directory contains a link"));
                }
                let relative_path = name(&directory, &path)?;
                relative(&relative_path)?;
                if !folded.insert(relative_path.to_lowercase()) {
                    return Err(invalid(
                        "declared directory has a case-insensitive path collision",
                    ));
                }
                if metadata.is_dir() {
                    if policy == DirectoryPolicy::CargoPackage
                        && current == directory
                        && matches!(entry.file_name().to_str(), Some(".git" | "target"))
                    {
                        continue;
                    }
                    entries.insert(relative_path, None);
                    pending.push(path);
                } else if metadata.is_file() {
                    let identity = FileIdentity::read(&path, PAYLOAD_LIMIT)?;
                    total = total
                        .checked_add(identity.bytes)
                        .ok_or_else(|| invalid("directory payload size overflow"))?;
                    if total > PAYLOAD_LIMIT {
                        return Err(invalid("directory payload exceeds 1 GiB"));
                    }
                    files.push((name(root, &path)?, identity));
                    entries.insert(relative_path, Some(identity));
                } else {
                    return Err(invalid("declared directory contains a special file"));
                }
                if entries.len() > FILE_LIMIT {
                    return Err(invalid("directory membership exceeds 4096 records"));
                }
            }
        }
        let mut hash = moirai_crypto::Sha256::new();
        for (path, identity) in &entries {
            hash.update(&[u8::from(identity.is_some())]);
            hash.update(
                &u32::try_from(path.len())
                    .map_err(|_| invalid("directory path length overflow"))?
                    .to_be_bytes(),
            );
            hash.update(path.as_bytes());
            if let Some(identity) = identity {
                hash.update(&identity.bytes.to_be_bytes());
                hash.update(identity.digest.as_bytes());
            }
        }
        files.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(Self {
            digest: Digest::from_bytes(hash.finalize()),
            files,
        })
    }
    /// Canonical membership digest, independent of host paths and timestamps.
    #[must_use]
    pub const fn digest(&self) -> Digest {
        self.digest
    }
    /// Exact owner-relative file names and computed identities.
    #[must_use]
    pub fn files(&self) -> &[(String, FileIdentity)] {
        &self.files
    }
}
fn name(root: &Path, path: &Path) -> io::Result<String> {
    let components = path
        .strip_prefix(root)
        .map_err(|_| invalid("directory member escapes owner"))?
        .components()
        .map(|part| {
            part.as_os_str()
                .to_str()
                .ok_or_else(|| invalid("directory member is not UTF-8"))
        })
        .collect::<io::Result<Vec<_>>>()?;
    Ok(components.join("/"))
}
