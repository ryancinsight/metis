//! A compiler unit owns primary outputs and their debug/import companions.
//!
//! Cargo 1.97 `target_info::file_types` appends auxiliary/debug outputs to
//! primary representations. They share the unit's source dependencies; a PDB
//! or import library never introduces a separate dependency record.
use crate::Result;
use serde_json::Value;
use std::{collections::BTreeSet, path::PathBuf};

pub(super) mod records;

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Unit {
    pub(super) target: String,
    pub(super) custom_build: bool,
    pub(super) cdylib: bool,
    pub(super) dylib: bool,
    pub(super) primary: Vec<Primary>,
    pub(super) companions: Vec<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Primary {
    pub(super) path: PathBuf,
    pub(super) dependency: PathBuf,
    pub(super) metadata: bool,
}

impl Unit {
    pub(super) fn parse(message: &Value) -> Result<Self> {
        let target = message.get("target").ok_or("compiler event omits target")?;
        let name = target
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .ok_or("compiler target omits its logical name")?;
        let kinds = strings(target, "kind")?;
        let types = strings(target, "crate_types")?;
        let custom_build = kinds == BTreeSet::from(["custom-build"]);
        if custom_build {
            if types != BTreeSet::from(["bin"]) {
                return Err("custom-build target must have the binary crate type".into());
            }
        } else if !kinds.iter().chain(&types).all(|kind| {
            matches!(
                *kind,
                "lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro"
            )
        }) {
            return Err("embedded compiler event has an unsupported target role".into());
        }
        let mut unit = Self {
            target: name.into(),
            custom_build,
            cdylib: types.contains("cdylib"),
            dylib: types.contains("dylib"),
            primary: Vec::new(),
            companions: Vec::new(),
        };
        let filenames = message
            .get("filenames")
            .and_then(Value::as_array)
            .filter(|files| !files.is_empty())
            .ok_or("compiler event omits output filenames")?;
        if filenames.len() > metis_core::embedded_assets::FILE_LIMIT {
            return Err("compiler output count exceeds the canonical input budget".into());
        }
        let mut seen = BTreeSet::new();
        for filename in filenames {
            let filename = filename
                .as_str()
                .ok_or("compiler output filename is not text")?;
            let path = PathBuf::from(filename);
            if !path.is_absolute() || !seen.insert(path.clone()) {
                return Err("compiler output is relative or duplicated".into());
            }
            let basename = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("compiler output has no Unicode basename")?;
            let (stem, prefix, companion) = representation(basename, &types, custom_build)?;
            let logical = logical_stem(stem, name, prefix, custom_build)?;
            if companion {
                unit.companions.push(path);
            } else {
                let dependency = path
                    .parent()
                    .ok_or("compiler primary has no directory")?
                    .join(format!("{logical}.d"));
                let metadata = path.extension() == Some(std::ffi::OsStr::new("rmeta"));
                unit.primary.push(Primary {
                    path,
                    dependency,
                    metadata,
                });
            }
        }
        if unit.primary.is_empty() {
            return Err("compiler event reports companions without a primary output".into());
        }
        Ok(unit)
    }
}

fn strings<'value>(value: &'value Value, key: &str) -> Result<BTreeSet<&'value str>> {
    let values = value
        .get(key)
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
        .ok_or_else(|| format!("compiler target omits {key}"))?;
    let mut result = BTreeSet::new();
    for value in values {
        let text = value
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("compiler target {key} contains a non-name"))?;
        if !result.insert(text) {
            return Err(format!("compiler target {key} repeats a role").into());
        }
    }
    Ok(result)
}

#[derive(Clone, Copy)]
enum Prefix {
    Required,
    Absent,
    Either,
}

fn representation<'name>(
    name: &'name str,
    types: &BTreeSet<&str>,
    custom: bool,
) -> Result<(&'name str, Prefix, bool)> {
    let dynamic = types
        .iter()
        .any(|kind| matches!(*kind, "dylib" | "cdylib" | "proc-macro"));
    // Longest compound suffixes precede their terminal extensions. Cargo
    // classifies .dSYM as debug information even though it is a directory.
    for (suffix, prefix, allowed) in [
        (".dll.lib", Prefix::Absent, dynamic),
        (".dll.exp", Prefix::Absent, dynamic),
        (".dll.a", Prefix::Required, dynamic),
        (".dylib.dSYM", Prefix::Required, dynamic),
        (".dylib.dwp", Prefix::Required, dynamic),
        (".so.dwp", Prefix::Required, dynamic),
        (".dll.dwp", Prefix::Either, dynamic),
        (".pdb", Prefix::Absent, dynamic || custom),
        (".dSYM", Prefix::Absent, custom),
        (".dwp", Prefix::Absent, custom),
    ] {
        if let Some(stem) = name.strip_suffix(suffix) {
            return if allowed {
                Ok((stem, prefix, true))
            } else {
                Err("compiler companion contradicts its target role".into())
            };
        }
    }
    if custom {
        return Ok((
            name.strip_suffix(".exe").unwrap_or(name),
            Prefix::Absent,
            false,
        ));
    }
    for (suffix, prefix, allowed) in [
        (".rmeta", Prefix::Required, true),
        (
            ".rlib",
            Prefix::Required,
            types.contains("lib") || types.contains("rlib"),
        ),
        (".dylib", Prefix::Required, dynamic),
        (".so", Prefix::Required, dynamic),
        (".dll", Prefix::Either, dynamic),
        (".wasm", Prefix::Absent, types.contains("cdylib")),
        (".a", Prefix::Required, types.contains("staticlib")),
        (".lib", Prefix::Absent, types.contains("staticlib")),
    ] {
        if let Some(stem) = name.strip_suffix(suffix) {
            return if allowed {
                Ok((stem, prefix, false))
            } else {
                Err("compiler primary output contradicts its target role".into())
            };
        }
    }
    Err("compiler output has an unsupported representation".into())
}

fn logical_stem<'stem>(
    stem: &'stem str,
    target: &str,
    prefix: Prefix,
    custom: bool,
) -> Result<&'stem str> {
    let logical = target.replace('-', "_");
    let matches = |candidate: &str| {
        candidate == logical
            || (custom && candidate == target)
            || candidate.strip_prefix(&logical).is_some_and(|suffix| {
                suffix.strip_prefix('-').is_some_and(|hash| {
                    !hash.is_empty() && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
            })
    };
    let admitted = match prefix {
        Prefix::Required => stem
            .strip_prefix("lib")
            .filter(|candidate| matches(candidate)),
        Prefix::Absent => matches(stem).then_some(stem),
        Prefix::Either => {
            // The two representations are disjoint under exact logical-name
            // boundaries. A logical `lib_owner` is never shortened to `owner`.
            matches(stem).then_some(stem).or_else(|| {
                stem.strip_prefix("lib")
                    .filter(|candidate| matches(candidate))
            })
        }
    };
    admitted.ok_or_else(|| "compiler output basename differs from its logical target".into())
}
