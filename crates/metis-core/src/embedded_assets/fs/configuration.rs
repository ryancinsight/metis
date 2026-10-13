//! Cargo config includes retain their defining path and are bounded before parse.
use super::super::{FILE_LIMIT, MANIFEST_LIMIT, inventory::invalid};
use super::{FileIdentity, canonical_file};
use std::{
    collections::BTreeSet,
    env, io,
    path::{Path, PathBuf},
};
mod compiler;
mod environment;
#[cfg(test)]
mod tests;
pub use compiler::{codegen_environment_keys, compiler_command, verify_compiler_pin};
pub use environment::build_environment_value;

/// Reads an environment value without treating non-Unicode values as absence.
///
/// # Errors
/// Rejects non-Unicode values because inventory text has a canonical UTF-8 codec.
pub fn environment_value(key: &str) -> io::Result<Option<String>> {
    match env::var(key) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error @ env::VarError::NotUnicode(_)) => {
            Err(invalid(format!("build environment {key}: {error}")))
        }
    }
}

/// Resolves the complete include closure in Cargo precedence order.
///
/// Every included config retains its original location, which determines relative
/// paths. Present optional includes are admitted. An absent optional include is
/// rejected because this inventory admits real files rather than silently omitting
/// a future compiler input. Cycles, links and excessive closures are rejected.
///
/// # Errors
/// Returns malformed, absent, linked, cyclic or excessive configuration errors.
pub fn configuration_inputs(path: &Path) -> io::Result<Vec<PathBuf>> {
    enum Step {
        Visit(PathBuf),
        Finish(PathBuf),
    }
    let mut inputs = Vec::new();
    let mut active = BTreeSet::new();
    let mut pending = vec![Step::Visit(path.to_path_buf())];
    let mut scheduled = 1_usize;
    while let Some(step) = pending.pop() {
        match step {
            Step::Finish(path) => {
                active.remove(&path);
                inputs.push(path);
            }
            Step::Visit(path) => {
                let path = canonical_file(&path)?;
                if !active.insert(path.clone()) {
                    return Err(invalid("Cargo configuration include cycle"));
                }
                let includes = included_paths(&path, &document(&path)?)?;
                scheduled = scheduled
                    .checked_add(includes.len())
                    .ok_or_else(|| invalid("Cargo configuration closure count overflow"))?;
                if scheduled > FILE_LIMIT {
                    return Err(invalid(
                        "Cargo configuration closure exceeds the record budget",
                    ));
                }
                pending.push(Step::Finish(path));
                pending.extend(includes.into_iter().rev().map(Step::Visit));
            }
        }
    }
    Ok(inputs)
}

fn included_paths(path: &Path, document: &toml::Value) -> io::Result<Vec<PathBuf>> {
    let Some(include) = document.get("include") else {
        return Ok(Vec::new());
    };
    let declarations = include
        .as_array()
        .ok_or_else(|| invalid("Cargo config include must be an array"))?;
    if declarations.len() > FILE_LIMIT {
        return Err(invalid(
            "Cargo configuration includes exceed the record budget",
        ));
    }
    let mut inputs = Vec::new();
    for declaration in declarations {
        let include = if let Some(path) = declaration.as_str() {
            path
        } else {
            let table = declaration
                .as_table()
                .ok_or_else(|| invalid("Cargo config include must contain paths or tables"))?;
            if table.keys().any(|key| key != "path" && key != "optional") {
                return Err(invalid("Cargo config include table has unknown fields"));
            }
            if table
                .get("optional")
                .is_some_and(|value| value.as_bool().is_none())
            {
                return Err(invalid("Cargo config include optional flag is not boolean"));
            }
            table
                .get("path")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| invalid("Cargo config include table omits path"))?
        };
        if Path::new(include)
            .extension()
            .is_none_or(|extension| extension != "toml")
        {
            return Err(invalid("Cargo config includes require .toml paths"));
        }
        inputs.push(
            path.parent()
                .ok_or_else(|| invalid("Cargo config has no parent"))?
                .join(include),
        );
    }
    Ok(inputs)
}

pub(super) fn document(path: &Path) -> io::Result<toml::Value> {
    let bytes = FileIdentity::read(path, MANIFEST_LIMIT)?.verified_bytes(path, MANIFEST_LIMIT)?;
    toml::from_str(
        std::str::from_utf8(&bytes)
            .map_err(|error| invalid(format!("Cargo configuration UTF-8: {error}")))?,
    )
    .map_err(|error| invalid(format!("Cargo configuration TOML: {error}")))
}

pub(super) fn target(path: &Path) -> io::Result<Option<PathBuf>> {
    let mut target = None;
    for input in configuration_inputs(path)? {
        if let Some(value) = document(&input)?
            .get("build")
            .and_then(|build| build.get("target-dir"))
        {
            let value = value
                .as_str()
                .ok_or_else(|| invalid("Cargo build target-dir is not a string"))?;
            let base = input
                .parent()
                .and_then(Path::parent)
                .ok_or_else(|| invalid("Cargo config has no relative-path base"))?;
            target = Some(base.join(value));
        }
    }
    Ok(target)
}
