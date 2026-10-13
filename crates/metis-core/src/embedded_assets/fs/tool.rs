//! Actual executable selection resolves recognized rustup proxies through rustup.
use super::super::{PAYLOAD_LIMIT, ToolKind, inventory::invalid};
use super::{FileIdentity, TOOL_IDENTITY_DEADLINE, capture_command};
use std::{
    io,
    path::{Path, PathBuf},
};
#[cfg(test)]
mod tests;

/// Resolves the actual selected executable, including an actual rustup proxy.
///
/// The current workspace selects rustup's repository/toolchain context. Proxy
/// recognition compares actual executable bytes to the current rustup binary;
/// an arbitrary wrapper is not treated as a proxy or accepted by its basename.
///
/// # Errors
/// Rejects absent commands, unresolved wrappers, malformed/relative rustup
/// output and unsuccessful/overflowing/timed-out native tool queries.
pub fn resolve_tool(kind: ToolKind, command: &Path, workspace: &Path) -> io::Result<PathBuf> {
    let selected = executable(command)?;
    if kind == ToolKind::WasmBindgen {
        return Ok(selected);
    }
    let rustup = match executable(Path::new("rustup")) {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(selected),
        Err(error) => return Err(error),
    };
    if FileIdentity::read(&selected, PAYLOAD_LIMIT)? != FileIdentity::read(&rustup, PAYLOAD_LIMIT)?
    {
        return Ok(selected);
    }
    let name = match kind {
        ToolKind::Cargo => "cargo",
        ToolKind::Rustc => "rustc",
        ToolKind::WasmBindgen => "wasm-bindgen",
    };
    let reported = capture_command(
        &rustup,
        &["which".into(), name.into()],
        TOOL_IDENTITY_DEADLINE,
        &[],
        Some(workspace),
    )?;
    let text = std::str::from_utf8(&reported)
        .map_err(|error| invalid(format!("rustup tool path UTF-8: {error}")))?
        .trim();
    let path = Path::new(text);
    if !path.is_absolute() {
        return Err(invalid("rustup returned a nonabsolute selected executable"));
    }
    let resolved = executable(path)?;
    if resolved == rustup {
        return Err(invalid(
            "rustup did not resolve its proxy to the selected tool",
        ));
    }
    Ok(resolved)
}
fn executable(command: &Path) -> io::Result<PathBuf> {
    if !command.is_absolute() && command.components().count() > 1 {
        return Err(invalid(
            "explicit tool paths must be absolute; relative invocation context is not an executable identity",
        ));
    }
    let selected = if command.is_absolute() || command.components().count() > 1 {
        command.to_path_buf()
    } else {
        let search = std::env::var_os("PATH")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "tool PATH is absent"))?;
        let name = command
            .to_str()
            .ok_or_else(|| invalid("tool command is not UTF-8"))?;
        let suffix = if Path::new(name).extension().is_some() {
            ""
        } else {
            std::env::consts::EXE_SUFFIX
        };
        std::env::split_paths(&search)
            .map(|directory| directory.join(format!("{name}{suffix}")))
            .find(|candidate| candidate.is_file())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("required tool unavailable on PATH: {name}"),
                )
            })?
    };
    // Rustup intentionally exposes proxies as symlinks on POSIX and copied
    // executable images on Windows. Only the final resolved file is admitted.
    let selected = selected.canonicalize()?;
    FileIdentity::read(&selected, PAYLOAD_LIMIT)?;
    Ok(selected)
}
