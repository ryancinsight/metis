//! Build and distribute applications from a single validated manifest.
mod build;
mod manifest;
mod process;
#[cfg(windows)]
#[expect(
    unsafe_code,
    reason = "Native Windows Installer ABI is isolated behind checked RAII handles"
)]
mod windows;

use std::path::Path;

/// The CLI's cold error-reporting boundary preserves concrete source errors.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const HELP: &str = "Métis application tooling

Usage:
  metis build MANIFEST OUTPUT       Build and stage a portable application
  metis package MANIFEST OUTPUT     Build portable application and Windows MSI
  metis --help                     Show this help

MANIFEST is a versioned metis.json file. OUTPUT must not exist.
Builds use release mode, Cargo --locked and actual compiler artifact messages.
The first process/installer workflow requires Windows; no signing or publication
is performed. See the distribution manual for installation and target limits.
";

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.is_empty() || args.as_slice() == ["--help"] {
        print!("{HELP}");
        return Ok(());
    }
    let [action, input, output] = args.as_slice() else {
        return Err(HELP.into());
    };
    let installer = match action.to_str() {
        Some("build") => build::OutputKind::Portable,
        Some("package") => build::OutputKind::Installer,
        _ => return Err(HELP.into()),
    };
    build::application(Path::new(input), Path::new(output), installer)
}
