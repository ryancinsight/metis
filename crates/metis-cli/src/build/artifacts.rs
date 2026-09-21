use crate::{Result, manifest::Application};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub(super) fn targets(
    metadata: &[u8],
    application: &Application,
) -> Result<BTreeMap<String, String>> {
    let document: serde_json::Value = serde_json::from_slice(metadata)?;
    let packages = document
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or("Cargo metadata omitted packages")?;
    let members = document
        .get("workspace_members")
        .and_then(serde_json::Value::as_array)
        .ok_or("Cargo metadata omitted workspace membership")?;
    let mut selected = BTreeMap::new();
    for binary in &application.binaries {
        let mut candidates = packages.iter().filter(|package| {
            package.get("name").and_then(serde_json::Value::as_str) == Some(&binary.package)
                && package.get("id").is_some_and(|id| members.contains(id))
        });
        let package = candidates
            .next()
            .ok_or("declared binary package is not a workspace member")?;
        if candidates.next().is_some() {
            return Err("declared workspace package is ambiguous".into());
        }
        let targets = package
            .get("targets")
            .and_then(serde_json::Value::as_array)
            .ok_or("Cargo package omitted targets")?;
        if !targets.iter().any(|target| {
            target.get("name").and_then(serde_json::Value::as_str) == Some(&binary.bin)
                && target
                    .get("kind")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some("bin")))
        }) {
            return Err("declared binary does not belong to its selected workspace package".into());
        }
        let id = package
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or("Cargo package omitted identity")?;
        selected.insert(binary.bin.clone(), id.to_owned());
    }
    Ok(selected)
}

pub(super) fn artifacts(
    messages: &[u8],
    selected: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, PathBuf>> {
    let mut artifacts = BTreeMap::new();
    for line in messages
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let message: serde_json::Value = serde_json::from_slice(line)?;
        if message.get("reason").and_then(serde_json::Value::as_str) != Some("compiler-artifact") {
            continue;
        }
        let target = message
            .get("target")
            .ok_or("compiler artifact has no target")?;
        let Some(name) = target.get("name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if !selected.contains_key(name) {
            continue;
        }
        if !target
            .get("kind")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|kinds| kinds.iter().any(|value| value.as_str() == Some("bin")))
        {
            continue;
        }
        if message
            .get("package_id")
            .and_then(serde_json::Value::as_str)
            != selected.get(name).map(String::as_str)
        {
            return Err(
                "compiler artifact package identity differs from the declared binary owner".into(),
            );
        }
        let path = message
            .get("executable")
            .and_then(serde_json::Value::as_str)
            .ok_or("compiler did not produce the selected executable")?;
        if !Path::new(path).is_absolute() {
            return Err("Cargo executable artifact must be absolute".into());
        }
        if artifacts
            .insert(name.to_owned(), PathBuf::from(path))
            .is_some()
        {
            return Err("ambiguous duplicate compiler executable artifact".into());
        }
    }
    for name in selected.keys() {
        if !artifacts.contains_key(name) {
            return Err(format!("Cargo did not report required binary {name}").into());
        }
    }
    Ok(artifacts)
}
