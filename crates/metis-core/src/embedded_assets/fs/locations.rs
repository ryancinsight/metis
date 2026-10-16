//! Cargo-owned local locations have one resolver for producer and consumer.
use super::super::inventory::invalid;
use super::linked;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Effective local Cargo home and target location, excluding source-patch policy.
#[derive(Debug)]
pub struct CargoLocations {
    home: PathBuf,
    target: PathBuf,
    configured_target: bool,
}
impl CargoLocations {
    /// Resolves current explicit environment and nearest Cargo target setting.
    ///
    /// On Windows, Cargo's user home uses `USERPROFILE`; on other hosts it
    /// uses `HOME`. Relative environment/configuration paths resolve before
    /// a subprocess changes working directory.
    ///
    /// # Errors
    /// Rejects absent home, malformed configuration and linked config files.
    pub fn current(workspace: &Path) -> io::Result<Self> {
        let home = if let Some(home) = std::env::var_os("CARGO_HOME") {
            PathBuf::from(home)
        } else {
            let variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
            PathBuf::from(
                std::env::var_os(variable)
                    .ok_or_else(|| invalid(format!("Cargo user home is absent: {variable}")))?,
            )
            .join(".cargo")
        };
        let home = super::canonical_directory(&home)?;
        let mut configured = std::env::var_os("CARGO_TARGET_DIR")
            .or_else(|| std::env::var_os("CARGO_BUILD_TARGET_DIR"))
            .map(PathBuf::from);
        if configured.is_none() {
            for directory in workspace.ancestors() {
                if let Some(path) = configuration(&directory.join(".cargo"))?
                    && let Some(target) = super::configuration::target(&path)?
                {
                    configured = Some(target);
                    break;
                }
            }
        }
        if configured.is_none()
            && let Some(path) = configuration(&home)?
        {
            configured = super::configuration::target(&path)?;
        }
        let configured_target = configured.is_some();
        let target = configured.unwrap_or_else(|| workspace.join("target"));
        let target = if target.is_absolute() {
            target
        } else {
            std::env::current_dir()?.join(target)
        };
        Ok(Self {
            home,
            target,
            configured_target,
        })
    }
    /// Canonical Cargo source/cache home.
    #[must_use]
    pub fn cargo_home(&self) -> &Path {
        &self.home
    }
    /// Absolute effective target directory; Cargo creates it during compilation.
    #[must_use]
    pub fn target_directory(&self) -> &Path {
        &self.target
    }
    /// Explicit target override to preserve across neutral subprocess invocation.
    #[must_use]
    pub fn target_override(&self) -> Option<&Path> {
        self.configured_target.then_some(&self.target)
    }
}
/// Finds the active Cargo config file, preserving legacy-name precedence.
///
/// # Errors
/// Rejects linked/special config files and contextual filesystem errors.
pub fn configuration(directory: &Path) -> io::Result<Option<PathBuf>> {
    for name in ["config", "config.toml"] {
        let path = directory.join(name);
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if !metadata.is_file() || linked(&metadata) {
                    return Err(invalid(
                        "Cargo configuration is not an unlinked regular file",
                    ));
                }
                return Ok(Some(path));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(None)
}
