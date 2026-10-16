//! Compiler selection and codegen environment keys have one authoritative owner.
use super::{configuration_inputs, document, environment_value};
use crate::embedded_assets::{configuration, inventory::invalid};
use std::{collections::BTreeSet, ffi::OsString, io, path::Path};

/// Selects Cargo's actual Rust compiler under the prepared workspace context.
///
/// Explicit RUSTC wins over Cargo environment configuration and file settings.
/// Config-relative paths retain their defining file's base. Compiler wrappers
/// fail explicitly: the fixed three-tool contract cannot attest their executable
/// closure and must not silently ignore an active wrapper.
///
/// # Errors
/// Rejects malformed config, non-Unicode commands and active compiler wrappers.
pub fn compiler_command(workspace: &Path, cargo_home: &Path) -> io::Result<OsString> {
    for (environment, setting) in [
        ("RUSTC_WRAPPER", "rustc-wrapper"),
        ("RUSTC_WORKSPACE_WRAPPER", "rustc-workspace-wrapper"),
    ] {
        let value = command(environment, setting, workspace, cargo_home)?;
        if value.is_some_and(|value| !value.is_empty()) {
            return Err(invalid(
                "active compiler wrappers are outside the three-tool inventory contract",
            ));
        }
    }
    Ok(command("RUSTC", "rustc", workspace, cargo_home)?.unwrap_or_else(|| "rustc".into()))
}

fn command(
    environment: &str,
    setting: &str,
    workspace: &Path,
    cargo_home: &Path,
) -> io::Result<Option<OsString>> {
    if let Some(value) = environment_value(environment)? {
        return Ok(Some(value.into()));
    }
    if let Some(value) = environment_value(&format!(
        "CARGO_BUILD_{}",
        setting.replace('-', "_").to_ascii_uppercase()
    ))? {
        return Ok(Some(value.into()));
    }
    let mut selected = None;
    for root in [cargo_home.to_path_buf(), workspace.join(".cargo")] {
        if let Some(path) = configuration(&root)? {
            for input in configuration_inputs(&path)? {
                if let Some(value) = document(&input)?
                    .get("build")
                    .and_then(|build| build.get(setting))
                {
                    let value = value
                        .as_str()
                        .ok_or_else(|| invalid("Cargo compiler command is not text"))?;
                    let path = Path::new(value);
                    selected = Some(if path.is_absolute() || path.components().count() <= 1 {
                        path.as_os_str().to_owned()
                    } else {
                        input
                            .parent()
                            .and_then(Path::parent)
                            .ok_or_else(|| invalid("compiler command has no config path base"))?
                            .join(path)
                            .into_os_string()
                    });
                }
            }
        }
    }
    Ok(selected)
}

/// Sorted compiler configuration keys, including absence and current target keys.
///
/// Known flags record absence so a newly defined value cannot evade stale checks.
/// New profile or target rustflags/linker keys enter the same closed consumer
/// comparison. Credential/registry variables are outside codegen ownership.
#[must_use]
pub fn codegen_environment_keys() -> BTreeSet<String> {
    let mut keys = BTreeSet::from([
        "RUSTFLAGS".into(),
        "CARGO_ENCODED_RUSTFLAGS".into(),
        "RUSTC_WRAPPER".into(),
        "RUSTC_WORKSPACE_WRAPPER".into(),
        "CARGO_INCREMENTAL".into(),
        "CI".into(),
        "CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS".into(),
        "CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER".into(),
    ]);
    for suffix in [
        "RUSTC",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTFLAGS",
        "INCREMENTAL",
        "DEP_INFO_BASEDIR",
        "TARGET",
        "BUILD_DIR",
    ] {
        keys.insert(format!("CARGO_BUILD_{suffix}"));
    }
    for suffix in [
        "OPT_LEVEL",
        "DEBUG",
        "SPLIT_DEBUGINFO",
        "STRIP",
        "DEBUG_ASSERTIONS",
        "OVERFLOW_CHECKS",
        "LTO",
        "PANIC",
        "INCREMENTAL",
        "CODEGEN_UNITS",
        "RPATH",
    ] {
        keys.insert(format!("CARGO_PROFILE_RELEASE_{suffix}"));
        keys.insert(format!("CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_{suffix}"));
    }
    for (key, _) in std::env::vars_os() {
        if let Some(key) = key.to_str()
            && (key.starts_with("CARGO_PROFILE_")
                || (key.starts_with("CARGO_TARGET_")
                    && (key.ends_with("_RUSTFLAGS") || key.ends_with("_LINKER"))))
        {
            keys.insert(key.to_owned());
        }
    }
    keys
}

/// Verifies the actual reported compiler release against the exact repository pin.
///
/// Rustup selectors may include a host target or use a short release channel; the
/// executable/version boundary owns identity, so selector text is not compared.
///
/// # Errors
/// Rejects malformed version or pin and a compiler release outside the pin.
pub fn verify_compiler_pin(version: &str, workspace: &Path) -> io::Result<()> {
    let pin = workspace.join("rust-toolchain.toml");
    if !pin.try_exists()? {
        return Ok(());
    }
    let document = document(&pin)?;
    let channel = document
        .get("toolchain")
        .and_then(|toolchain| toolchain.get("channel"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| invalid("Rust toolchain pin omits channel"))?;
    let reported = version
        .strip_prefix("rustc ")
        .and_then(|text| text.split_whitespace().next())
        .ok_or_else(|| invalid("Rust compiler version has no canonical release identity"))?;
    if reported != channel {
        return Err(invalid(
            "prepared Rust compiler differs from the exact repository toolchain pin",
        ));
    }
    Ok(())
}
