use crate::{Result, manifest::Application};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

/// Cargo's `compiler-artifact` messages from a `--message-format=json` stream.
fn compiler_artifacts(messages: &[u8]) -> impl Iterator<Item = Result<serde_json::Value>> {
    messages
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .filter_map(
            |line| match serde_json::from_slice::<serde_json::Value>(line) {
                Ok(message)
                    if message.get("reason").and_then(serde_json::Value::as_str)
                        == Some("compiler-artifact") =>
                {
                    Some(Ok(message))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error.into())),
            },
        )
}

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
    for message in compiler_artifacts(messages) {
        let message = message?;
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

/// The `cdylib` target a browser application compiles, and the
/// `wasm-bindgen` release its dependency graph locks.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct ModuleTarget {
    pub(super) package_id: String,
    pub(super) target: String,
    pub(super) bindgen_version: String,
}

/// Selects `package`'s `cdylib` target from full `cargo metadata` output,
/// with the one `wasm-bindgen` version in that package's dependency closure;
/// the generating CLI must match it exactly.
pub(super) fn module_target(metadata: &[u8], package: &str) -> Result<ModuleTarget> {
    let document: serde_json::Value = serde_json::from_slice(metadata)?;
    let packages = document
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or("Cargo metadata omitted packages")?;
    let members = document
        .get("workspace_members")
        .and_then(serde_json::Value::as_array)
        .ok_or("Cargo metadata omitted workspace membership")?;
    let mut candidates = packages.iter().filter(|candidate| {
        candidate.get("name").and_then(serde_json::Value::as_str) == Some(package)
            && candidate.get("id").is_some_and(|id| members.contains(id))
    });
    let owner = candidates
        .next()
        .ok_or_else(|| format!("{package} is not a workspace member"))?;
    if candidates.next().is_some() {
        return Err("declared workspace package is ambiguous".into());
    }
    let package_id = owner
        .get("id")
        .and_then(serde_json::Value::as_str)
        .ok_or("Cargo package omitted identity")?;
    let target = owner
        .get("targets")
        .and_then(serde_json::Value::as_array)
        .ok_or("Cargo package omitted targets")?
        .iter()
        .find(|target| kinds(target).any(|kind| kind == "cdylib"))
        .and_then(|target| target.get("name").and_then(serde_json::Value::as_str))
        .ok_or_else(|| format!("{package} has no cdylib target to compile to WebAssembly"))?;
    let versions = closure(&document, package_id)?
        .into_iter()
        .filter_map(|id| {
            packages
                .iter()
                .find(|candidate| candidate.get("id") == Some(id))
        })
        .filter(|candidate| {
            candidate.get("name").and_then(serde_json::Value::as_str) == Some("wasm-bindgen")
        })
        .filter_map(|candidate| candidate.get("version").and_then(serde_json::Value::as_str))
        .collect::<BTreeSet<_>>();
    let bindgen_version = match versions.into_iter().collect::<Vec<_>>().as_slice() {
        [version] => (*version).to_owned(),
        [] => return Err(format!("{package} does not depend on wasm-bindgen").into()),
        _ => return Err(format!("{package} resolves more than one wasm-bindgen").into()),
    };
    Ok(ModuleTarget {
        package_id: package_id.to_owned(),
        target: target.to_owned(),
        bindgen_version,
    })
}

/// Package ids reachable from `root` through the resolved dependency graph.
fn closure<'document>(
    document: &'document serde_json::Value,
    root: &str,
) -> Result<Vec<&'document serde_json::Value>> {
    let nodes = document
        .get("resolve")
        .and_then(|resolve| resolve.get("nodes"))
        .and_then(serde_json::Value::as_array)
        .ok_or("Cargo metadata omitted the resolved graph")?;
    let mut reached = Vec::new();
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        let node = nodes
            .iter()
            .find(|node| node.get("id").and_then(serde_json::Value::as_str) == Some(id))
            .ok_or("resolved graph omits a reachable package")?;
        let identity = node.get("id").ok_or("resolved node omitted identity")?;
        if reached.contains(&identity) {
            continue;
        }
        reached.push(identity);
        pending.extend(
            node.get("dependencies")
                .and_then(serde_json::Value::as_array)
                .ok_or("resolved node omitted dependencies")?
                .iter()
                .filter_map(serde_json::Value::as_str),
        );
    }
    Ok(reached)
}

/// The `.wasm` file Cargo reported for `module`'s target.
pub(super) fn module(messages: &[u8], module: &ModuleTarget) -> Result<PathBuf> {
    let mut found = None;
    for message in compiler_artifacts(messages) {
        let message = message?;
        let target = message
            .get("target")
            .ok_or("compiler artifact has no target")?;
        if message
            .get("package_id")
            .and_then(serde_json::Value::as_str)
            != Some(module.package_id.as_str())
            || target.get("name").and_then(serde_json::Value::as_str)
                != Some(module.target.as_str())
            || !kinds(target).any(|kind| kind == "cdylib")
        {
            continue;
        }
        let mut modules = message
            .get("filenames")
            .and_then(serde_json::Value::as_array)
            .ok_or("compiler artifact omitted its files")?
            .iter()
            .filter_map(serde_json::Value::as_str)
            .filter(|path| {
                Path::new(path)
                    .extension()
                    .is_some_and(|extension| extension == "wasm")
            });
        let path = modules
            .next()
            .ok_or("compiler did not produce a WebAssembly module")?;
        if modules.next().is_some() || found.is_some() {
            return Err("ambiguous duplicate WebAssembly module artifact".into());
        }
        if !Path::new(path).is_absolute() {
            return Err("Cargo module artifact must be absolute".into());
        }
        found = Some(PathBuf::from(path));
    }
    found.ok_or_else(|| format!("Cargo did not report the {} module", module.target).into())
}

fn kinds(target: &serde_json::Value) -> impl Iterator<Item = &str> {
    target
        .get("kind")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
}
