//! Capability-witnessed native file reads anchored below one trusted root.

use metis_core::capability::CapabilityScope;
use metis_core::host::VerifiedHostCapability;
use moirai_pal::fs::open_file_within_root;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Maximum bytes returned by one scoped native read.
pub const MAX_SCOPED_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// Native file provider bound to one host-selected directory.
///
/// The provider never accepts an unrestricted path. Each read starts from the
/// stored root and uses Moirai's directory-handle walk, which rejects parent
/// components, links and non-regular files before returning the opened handle.
/// A [`VerifiedHostCapability`] is required at the call site so authorization
/// cannot be omitted accidentally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedFileProvider {
    root: PathBuf,
}

impl ScopedFileProvider {
    /// Creates a provider for an existing directory.
    ///
    /// The root is retained as supplied and is checked again by the provider
    /// for every read. This keeps root replacement visible to the platform
    /// opener instead of converting a trusted path into a canonicalized alias.
    ///
    /// # Errors
    /// Returns the platform error when `root` cannot be inspected or is not a
    /// directory.
    pub fn new(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        if !fs::metadata(&root)?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "scoped file root must be a directory",
            ));
        }
        Ok(Self { root })
    }

    /// Returns the configured root for diagnostics and policy display.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Reads one regular file below the configured root.
    ///
    /// The capability witness must have been produced by the trusted
    /// [`metis_core::host::HostPolicy`] for `CapabilityScope::READ_FILE`.
    /// Witnesses do not extend token expiry; callers re-authorize before a
    /// later operation when the host session requires it.
    ///
    /// # Errors
    /// Returns a permission or path error for an escape, link, directory or
    /// missing file, an I/O error from the opened handle, or
    /// [`io::ErrorKind::InvalidData`] when the bounded read exceeds
    /// [`MAX_SCOPED_FILE_BYTES`].
    pub fn read(
        &self,
        _capability: &VerifiedHostCapability<{ CapabilityScope::READ_FILE.0 }>,
        path: impl AsRef<Path>,
    ) -> io::Result<Vec<u8>> {
        let file = open_file_within_root(path, &self.root)?;
        let size = file.metadata()?.len();
        if size > MAX_SCOPED_FILE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "scoped file exceeds the provider byte bound",
            ));
        }
        let capacity = usize::try_from(size).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "scoped file size is not representable on this target",
            )
        })?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(capacity)
            .map_err(|_| io::Error::other("scoped file allocation exceeds the provider budget"))?;
        file.take(MAX_SCOPED_FILE_BYTES.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if u64::try_from(bytes.len()).is_ok_and(|length| length > MAX_SCOPED_FILE_BYTES) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "scoped file grew beyond the provider byte bound",
            ));
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use metis_core::capability::{CapabilityGrantSpec, CapabilityScope};
    use metis_core::error::ErrorCode;
    use metis_core::host::{HostOrigin, HostPolicy, HostSessionId, WindowId};
    use std::time::{SystemTime, UNIX_EPOCH};

    const KEY: &[u8] = b"metis-scoped-file-test-key";

    fn root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock is after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "metis-scoped-file-{name}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn capability(
        scope: CapabilityScope,
    ) -> VerifiedHostCapability<{ CapabilityScope::READ_FILE.0 }> {
        let policy = HostPolicy::new(
            HostOrigin::parse("http://127.0.0.1:8080").expect("test origin"),
            WindowId::new(1).expect("test window"),
        );
        let session = HostSessionId::new([7; 16]).expect("test session");
        let context = policy.context_for(session);
        let token = context
            .issue_capability(
                CapabilityGrantSpec {
                    token_id: 1,
                    principal_id: session.as_bytes(),
                    scope,
                    issued_at_secs: 10,
                    duration_secs: 100,
                    nonce: 1,
                },
                KEY,
            )
            .expect("test token");
        policy
            .authorize::<{ CapabilityScope::READ_FILE.0 }>(&token, &context, 11, KEY)
            .expect("file capability")
    }

    fn remove_tree(path: &Path) {
        if path.exists() {
            fs::remove_dir_all(path).expect("test tree cleanup");
        }
    }

    #[test]
    fn reads_regular_file_below_root_with_file_capability() {
        let root = root("read");
        let nested = root.join("series");
        fs::create_dir_all(&nested).expect("test directory");
        fs::write(nested.join("slice.dcm"), b"dicom-bytes").expect("test file");
        let provider = ScopedFileProvider::new(&root).expect("provider");
        let bytes = provider
            .read(&capability(CapabilityScope::READ_FILE), "series/slice.dcm")
            .expect("scoped read");
        assert_eq!(bytes, b"dicom-bytes");
        remove_tree(&root);
    }

    #[test]
    fn rejects_escape_and_oversized_files() {
        let root = root("reject");
        fs::create_dir_all(&root).expect("test directory");
        fs::write(root.join("safe.dcm"), b"safe").expect("test file");
        fs::create_dir(root.join("folder")).expect("test directory");
        let large = fs::File::create(root.join("large.dcm")).expect("large test file");
        large
            .set_len(MAX_SCOPED_FILE_BYTES + 1)
            .expect("sparse test file size");
        let provider = ScopedFileProvider::new(&root).expect("provider");
        let capability = capability(CapabilityScope::READ_FILE);
        let escape = provider
            .read(&capability, "../safe.dcm")
            .expect_err("parent escape");
        assert_eq!(escape.kind(), io::ErrorKind::InvalidInput);
        let directory = provider
            .read(&capability, "folder")
            .expect_err("directory must be rejected");
        assert!(matches!(
            directory.kind(),
            io::ErrorKind::InvalidInput
                | io::ErrorKind::NotADirectory
                | io::ErrorKind::PermissionDenied
        ));
        let large = provider
            .read(&capability, "large.dcm")
            .expect_err("large file");
        assert_eq!(large.kind(), io::ErrorKind::InvalidData);
        remove_tree(&root);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_file_inside_root() {
        use std::os::unix::fs::symlink;

        let outside = root("symlink-target");
        let root = root("symlink");
        fs::create_dir_all(&root).expect("test directory");
        fs::create_dir_all(&outside).expect("target directory");
        fs::write(outside.join("outside.dcm"), b"outside").expect("target file");
        symlink(outside.join("outside.dcm"), root.join("selected.dcm")).expect("test symlink");
        let provider = ScopedFileProvider::new(&root).expect("provider");
        let error = provider
            .read(&capability(CapabilityScope::READ_FILE), "selected.dcm")
            .expect_err("symlink must be rejected");
        assert!(error.raw_os_error().is_some());
        remove_tree(&root);
        remove_tree(&outside);
    }

    #[test]
    fn host_policy_denies_file_witness_without_file_scope() {
        let policy = HostPolicy::new(
            HostOrigin::parse("http://127.0.0.1:8080").expect("test origin"),
            WindowId::new(1).expect("test window"),
        );
        let session = HostSessionId::new([8; 16]).expect("test session");
        let context = policy.context_for(session);
        let token = context
            .issue_capability(
                CapabilityGrantSpec {
                    token_id: 2,
                    principal_id: session.as_bytes(),
                    scope: CapabilityScope::UI_RENDER,
                    issued_at_secs: 10,
                    duration_secs: 100,
                    nonce: 2,
                },
                KEY,
            )
            .expect("test token");
        let error = policy
            .authorize::<{ CapabilityScope::READ_FILE.0 }>(&token, &context, 11, KEY)
            .expect_err("missing file scope");
        assert_eq!(error.code, ErrorCode::InsufficientScope);
    }
}
