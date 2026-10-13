//! Cargo child environment follows include precedence and force/relative rules.
use super::{configuration_inputs, document, environment_value};
use crate::embedded_assets::{configuration, inventory::invalid};
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};

#[derive(Default)]
struct Setting {
    value: Option<String>,
    force: bool,
    relative: bool,
    origin: PathBuf,
}

/// Resolves a compiler-input variable under the lock-authoritative Cargo context.
///
/// This context uses Cargo home followed by the owning workspace configuration;
/// stack ancestor source patches are excluded by the producer's neutral cwd.
/// Config `env` values enter the child environment: ambient values win unless
/// `force=true`, and `relative=true` resolves against the defining config base.
///
/// # Errors
/// Rejects malformed configuration or non-Unicode effective environment values.
pub fn build_environment_value(
    key: &str,
    workspace: &Path,
    cargo_home: &Path,
) -> io::Result<Option<String>> {
    let mut settings = BTreeMap::<String, Setting>::new();
    for root in [cargo_home.to_path_buf(), workspace.join(".cargo")] {
        if let Some(path) = configuration(&root)? {
            for input in configuration_inputs(&path)? {
                merge(&input, &mut settings)?;
            }
        }
    }
    let ambient = environment_value(key)?;
    let Some(setting) = settings.get(key) else {
        return Ok(ambient);
    };
    if !setting.force && ambient.is_some() {
        return Ok(ambient);
    }
    let value = setting
        .value
        .as_ref()
        .ok_or_else(|| invalid("Cargo env entry omits value"))?;
    if setting.relative {
        let base = setting
            .origin
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| invalid("Cargo env entry has no relative-path base"))?;
        let value = base.join(value);
        Ok(Some(
            value
                .to_str()
                .ok_or_else(|| invalid("Cargo relative env value is not UTF-8"))?
                .to_owned(),
        ))
    } else {
        Ok(Some(value.clone()))
    }
}

fn merge(input: &Path, settings: &mut BTreeMap<String, Setting>) -> io::Result<()> {
    let doc = document(input)?;
    let Some(entries) = doc.get("env") else {
        return Ok(());
    };
    for (key, value) in entries
        .as_table()
        .ok_or_else(|| invalid("Cargo env must be a table"))?
    {
        let setting = settings.entry(key.clone()).or_default();
        if let Some(value) = value.as_str() {
            *setting = Setting {
                value: Some(value.to_owned()),
                origin: input.to_path_buf(),
                ..Setting::default()
            };
            continue;
        }
        let table = value
            .as_table()
            .ok_or_else(|| invalid("Cargo env entry must be text or a table"))?;
        if table
            .keys()
            .any(|key| !matches!(key.as_str(), "value" | "force" | "relative"))
        {
            return Err(invalid("Cargo env entry has unknown fields"));
        }
        if let Some(value) = table.get("value") {
            setting.value = Some(
                value
                    .as_str()
                    .ok_or_else(|| invalid("Cargo env value is not text"))?
                    .to_owned(),
            );
            setting.origin = input.to_path_buf();
        }
        if let Some(force) = table.get("force") {
            setting.force = force
                .as_bool()
                .ok_or_else(|| invalid("Cargo env force is not boolean"))?;
        }
        if let Some(relative) = table.get("relative") {
            setting.relative = relative
                .as_bool()
                .ok_or_else(|| invalid("Cargo env relative is not boolean"))?;
        }
    }
    Ok(())
}
