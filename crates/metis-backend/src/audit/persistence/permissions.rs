use metis_core::error::Result;
use std::fs::File;
use std::path::Path;

#[cfg(unix)]
use super::storage_error;
#[cfg(unix)]
use metis_core::error::ErrorCode;
#[cfg(unix)]
use std::fs;

#[cfg(unix)]
pub(super) fn restrict_directory(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)
        .map_err(|_| storage_error(ErrorCode::IoError))?
        .permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).map_err(|_| storage_error(ErrorCode::IoError))
}

#[cfg(not(unix))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "share the fallible Unix permission API at the platform boundary"
)]
pub(super) const fn restrict_directory(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
pub(super) fn restrict_file(file: &File) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = file
        .metadata()
        .map_err(|_| storage_error(ErrorCode::IoError))?
        .permissions();
    permissions.set_mode(0o600);
    file.set_permissions(permissions)
        .map_err(|_| storage_error(ErrorCode::IoError))
}

#[cfg(not(unix))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "share the fallible Unix permission API at the platform boundary"
)]
pub(super) const fn restrict_file(_file: &File) -> Result<()> {
    Ok(())
}
