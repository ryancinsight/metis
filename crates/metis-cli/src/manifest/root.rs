//! Explicit configuration-to-owner selection shared by every CLI command.
//!
//! A locator has at most `PORTABLE_PATH_LIMIT` bytes and is either `.` or a
//! slash-separated sequence of portable normal components and `..`. Only this
//! locator admits parent components: every payload, Cargo manifest, icon and
//! frontend path keeps the core `relative` contract beneath the resolved owner.
//!
//! Explicit resolution rejects linked manifest/root components before canonicalization.
//! The declared root must be an existing directory containing the canonical
//! manifest. Hence an explicit locator selects an ancestor ownership domain,
//! never a sibling whose paths could silently reinterpret the application.
//! Omission retains existing canonical manifest-parent ownership and admission,
//! including resolution of previously supported manifest aliases.
use crate::Result;
use metis_core::embedded_assets::{
    PORTABLE_PATH_LIMIT, canonical_directory, canonical_file, relative,
};
use std::path::{Path, PathBuf};

pub(super) fn validate(locator: Option<&str>) -> Result<()> {
    if let Some(locator) = locator {
        if locator.is_empty() || locator.len() > PORTABLE_PATH_LIMIT {
            return Err("application_root must be a bounded directory locator".into());
        }
        if locator != "." {
            for component in locator.split('/') {
                if component != ".." {
                    relative(component)?;
                }
            }
        }
    }
    Ok(())
}

pub(super) fn resolve(manifest: &Path, locator: Option<&str>) -> Result<PathBuf> {
    let absolute = std::path::absolute(manifest)?;
    let manifest = if locator.is_some() {
        canonical_file(&absolute)?
    } else {
        absolute.canonicalize()?
    };
    let directory = manifest.parent().ok_or("manifest has no parent")?;
    let Some(locator) = locator else {
        return Ok(directory.to_path_buf());
    };
    let root = canonical_directory(&directory.join(locator))?;
    if !manifest.starts_with(&root) {
        return Err("manifest must remain beneath declared application_root".into());
    }
    Ok(root)
}
