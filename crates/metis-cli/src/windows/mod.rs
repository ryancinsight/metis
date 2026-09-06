//! Windows Installer packages use an embedded cabinet and standard MSI actions.
//!
//! No custom install/uninstall program executes: Windows Installer tracks each
//! component and owns transactional rollback. Related installations are detected
//! and rejected; the existing product must be removed before changing versions.
//!
//! Contracts: [per-user/no-elevation packages](https://learn.microsoft.com/en-us/windows/win32/msi/authoring-packages-without-the-uac-dialog-box),
//! [related-product detection](https://learn.microsoft.com/en-us/windows/win32/msi/upgrade-table),
//! and [standard action ordering](https://learn.microsoft.com/en-us/windows/win32/msi/installexecutesequence-table).
//! Native database/cabinet tests establish authored values; the separate bounded
//! install/run/uninstall workflow establishes actual Windows Installer behavior.
mod database;
mod encoding;
mod ffi;
mod package;
mod paths;
mod payload;
mod schema;
mod shortcut;

pub(crate) use package::{InstallerSpec, build, inspect};

#[cfg(test)]
mod tests;
