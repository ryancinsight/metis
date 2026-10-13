//! Cargo compiler/build-script events extend their canonical input domains.
use super::{Graph, InputSet, cargo_generated_environment, dep_info, dep_info_file};
use crate::{Result, tree};
use metis_core::embedded_assets::{DirectoryPolicy, FILE_LIMIT, OwnerBinding};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub(super) fn collect(
    messages: &[u8],
    target: &super::super::super::artifacts::ModuleTarget,
    graph: &Graph,
    input: &mut InputSet,
) -> Result<()> {
    for line in messages
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let message: Value = serde_json::from_slice(line)?;
        let Some(reason) = message.get("reason").and_then(Value::as_str) else {
            continue;
        };
        let package_id = message.get("package_id").and_then(Value::as_str);
        let Some((_, owner, root)) = graph
            .packages
            .iter()
            .find(|(id, _, _)| Some(id.as_str()) == package_id)
        else {
            continue;
        };
        match reason {
            "compiler-artifact" => compiler(&message, package_id, target, graph, input)?,
            "build-script-executed" => script(&message, owner, root, input)?,
            _ => {}
        }
        if input.files.len() > FILE_LIMIT {
            return Err("compiler input file count exceeds the canonical budget".into());
        }
    }
    Ok(())
}
fn compiler(
    message: &Value,
    package_id: Option<&str>,
    target: &super::super::super::artifacts::ModuleTarget,
    graph: &Graph,
    input: &mut InputSet,
) -> Result<()> {
    let paths = message
        .get("filenames")
        .and_then(Value::as_array)
        .ok_or("compiler artifact omitted filenames")?;
    let mut infos = BTreeSet::new();
    for file in paths.iter().filter_map(Value::as_str) {
        infos.insert(dep_info_file(Path::new(file))?);
    }
    for path in infos {
        let bytes = crate::bounded_read::read_file(
            &path,
            metis_core::embedded_assets::MANIFEST_LIMIT,
            "compiler dep-info exceeds 1 MiB",
        )?;
        input
            .files
            .extend(dep_info::paths(&bytes, &graph.workspace)?);
        for line in std::str::from_utf8(&bytes)?.lines() {
            if let Some(declaration) = line.strip_prefix("# env-dep:") {
                let key = declaration
                    .split('=')
                    .next()
                    .ok_or("empty compiler environment key")?;
                if !cargo_generated_environment(key) {
                    input.environment.insert(key.to_owned());
                }
            }
        }
    }
    if package_id == Some(target.package_id.as_str()) {
        for feature in message
            .get("features")
            .and_then(Value::as_array)
            .ok_or("compiler artifact omits features")?
            .iter()
            .filter_map(Value::as_str)
        {
            input.features.insert(feature.to_owned());
        }
    }

    Ok(())
}
fn script(message: &Value, owner: &str, root: &Path, input: &mut InputSet) -> Result<()> {
    let output = PathBuf::from(
        message
            .get("out_dir")
            .and_then(Value::as_str)
            .ok_or("build-script event omitted OUT_DIR")?,
    )
    .canonicalize()?;
    let build = output
        .parent()
        .ok_or("build-script OUT_DIR has no build instance")?;
    let instance = build
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("build-script instance is not UTF-8")?;
    input.owners.push(OwnerBinding {
        owner_id: format!("generated:{owner}::{instance}"),
        root: output.clone(),
    });
    input.owners.push(OwnerBinding {
        owner_id: format!("build:{owner}::{instance}"),
        root: build.to_path_buf(),
    });
    input.files.extend(tree::regular_files(&output, |_| false)?);
    let report = build.join("output");
    input.files.insert(report.clone());
    let bytes = crate::bounded_read::read_file(
        &report,
        metis_core::embedded_assets::MANIFEST_LIMIT,
        "build-script output exceeds 1 MiB",
    )?;
    let mut declarations = false;
    for line in std::str::from_utf8(&bytes)?.lines() {
        if let Some(path) = line
            .strip_prefix("cargo:rerun-if-changed=")
            .or_else(|| line.strip_prefix("cargo::rerun-if-changed="))
        {
            declarations = true;
            let declared_path = root.join(path);
            if declared_path.is_dir() {
                input.declared_directories.insert((
                    metis_core::embedded_assets::canonical_directory(&declared_path)?,
                    DirectoryPolicy::Declared,
                ));
            } else {
                input
                    .files
                    .insert(metis_core::embedded_assets::canonical_file(&declared_path)?);
            }
        }
        if let Some(key) = line
            .strip_prefix("cargo:rerun-if-env-changed=")
            .or_else(|| line.strip_prefix("cargo::rerun-if-env-changed="))
            && !cargo_generated_environment(key)
        {
            input.environment.insert(key.to_owned());
        }
    }
    // Cargo's default build-script invalidation owns the package
    // tree when no file rerun declarations are emitted.
    if !declarations {
        input
            .declared_directories
            .insert((root.to_path_buf(), DirectoryPolicy::CargoPackage));
    }

    Ok(())
}
