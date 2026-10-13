//! Verification phases enforce tool, file, configuration and directory coherence.
#[cfg(test)]
mod tests;
use super::super::{EmbeddedModuleInventory, PAYLOAD_LIMIT, inventory::invalid, source};
use super::{FileIdentity, LocalBindings};
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};
pub(super) fn bound_tools(
    inventory: &EmbeddedModuleInventory,
    bindings: &LocalBindings,
    workspace: &Path,
    rerun_paths: &mut Vec<PathBuf>,
) -> io::Result<()> {
    for (binding, tool) in bindings.tools.iter().zip(&inventory.spec().tools) {
        if binding.kind != tool.kind || binding.executable.canonicalize()? != binding.executable {
            return Err(invalid("tool binding is not canonical for its role"));
        }
        if FileIdentity::read(&binding.executable, PAYLOAD_LIMIT)? != tool.executable {
            return Err(invalid(format!(
                "prepared tool identity is stale: {}",
                binding.executable.display()
            )));
        }
        let version = super::capture_command(
            &binding.executable,
            &["--version".into()],
            super::TOOL_IDENTITY_DEADLINE,
            &[],
            Some(workspace),
        )?;
        let version = std::str::from_utf8(&version)
            .map_err(|error| invalid(format!("tool version UTF-8: {error}")))?
            .trim();
        if version != tool.version {
            return Err(invalid(format!(
                "prepared tool version is stale: {}",
                tool_environment(binding.kind)
            )));
        }
        if binding.kind == super::super::ToolKind::Rustc {
            super::verify_compiler_pin(version, workspace)?;
        }
        rerun_paths.push(binding.executable.clone());
    }
    Ok(())
}

// This phase has producer authority only. Native Cargo's compiler flags,
// selectors and Clippy wrapper do not describe the already compiled WASM module.
pub(super) fn producer_configuration(
    inventory: &EmbeddedModuleInventory,
    bindings: &LocalBindings,
    workspace: &Path,
    cargo_home: &Path,
) -> io::Result<()> {
    for binding in &bindings.tools {
        let environment = tool_environment(binding.kind);
        if let Some(current) = std::env::var_os(environment)
            && super::resolve_tool(binding.kind, Path::new(&current), workspace)?
                != binding.executable
        {
            return Err(invalid(format!(
                "current {environment} differs from prepared tool"
            )));
        }
    }
    for key in super::codegen_environment_keys() {
        if !inventory
            .spec()
            .configuration
            .environment
            .iter()
            .any(|(declared, _)| *declared == key)
        {
            return Err(invalid(format!(
                "current codegen environment has no recorded presence/absence: {key}"
            )));
        }
    }
    // Resolving this context also rejects active compiler wrappers rather than
    // treating their unrecorded executable bytes as a verified tool closure.
    super::compiler_command(workspace, cargo_home)?;
    for (key, value) in &inventory.spec().configuration.environment {
        if super::build_environment_value(key, workspace, cargo_home)? != *value {
            return Err(invalid(format!(
                "prepared build environment is stale: {key}"
            )));
        }
    }
    Ok(())
}

const fn tool_environment(kind: super::super::ToolKind) -> &'static str {
    match kind {
        super::super::ToolKind::Rustc => "RUSTC",
        super::super::ToolKind::Cargo => "CARGO",
        super::super::ToolKind::WasmBindgen => "WASM_BINDGEN",
    }
}
pub(super) fn inputs(
    inventory: &EmbeddedModuleInventory,
    bindings: &LocalBindings,
    workspace: &Path,
    cargo_home: &Path,
    rerun_paths: &mut Vec<PathBuf>,
) -> io::Result<BTreeMap<PathBuf, FileIdentity>> {
    let mut verified_inputs = std::collections::BTreeMap::new();
    for input in &inventory.spec().inputs {
        let binding = bindings
            .owners
            .iter()
            .find(|owner| owner.owner_id == input.owner_id)
            .ok_or_else(|| {
                invalid(format!(
                    "input has no local owner binding: {}",
                    input.owner_id
                ))
            })?;
        let path = source(&binding.root, &input.path)?;
        if FileIdentity::read(&path, PAYLOAD_LIMIT)? != input.identity {
            return Err(invalid(format!(
                "prepared compiler input is stale: {}",
                path.display()
            )));
        }
        verified_inputs.insert(path.clone(), input.identity);
        rerun_paths.push(path);
    }
    configuration_inputs(workspace, cargo_home, &verified_inputs, rerun_paths)?;
    Ok(verified_inputs)
}

fn configuration_inputs(
    workspace: &Path,
    cargo_home: &Path,
    verified_inputs: &BTreeMap<PathBuf, FileIdentity>,
    rerun_paths: &mut Vec<PathBuf>,
) -> io::Result<()> {
    let mut configurations = vec![workspace.join("rust-toolchain.toml")];
    for root in [workspace.join(".cargo"), cargo_home.to_path_buf()] {
        if let Some(path) = super::configuration(&root)? {
            configurations.extend(super::configuration_inputs(&path)?);
        }
        rerun_paths.extend([root.join("config"), root.join("config.toml")]);
    }
    for path in configurations {
        // Watching an absent config path for future Cargo rebuilds does not
        // declare its newly created bytes as an input of the prepared module.
        if path.try_exists()? && !verified_inputs.contains_key(&path) {
            return Err(invalid(format!(
                "current build configuration is not a declared input: {}",
                path.display()
            )));
        }
        rerun_paths.push(path);
    }
    Ok(())
}
pub(super) fn directories(
    inventory: &EmbeddedModuleInventory,
    bindings: &LocalBindings,
    verified_inputs: &BTreeMap<PathBuf, FileIdentity>,
    rerun_paths: &mut Vec<PathBuf>,
) -> io::Result<()> {
    for directory_record in &inventory.spec().directories {
        let binding = bindings
            .owners
            .iter()
            .find(|owner| owner.owner_id == directory_record.owner_id)
            .ok_or_else(|| invalid("directory has no admitted owner"))?;
        let snapshot = super::DirectorySnapshot::read(
            &binding.root,
            directory_record.path.as_deref(),
            directory_record.policy,
        )?;
        if snapshot.digest() != directory_record.digest {
            return Err(invalid("prepared input-directory membership is stale"));
        }
        for (path, identity) in snapshot.files() {
            if verified_inputs.get(&binding.root.join(path)) != Some(identity) {
                return Err(invalid(
                    "declared directory member is not a recorded compiler input",
                ));
            }
        }
        rerun_paths.push(
            directory_record
                .path
                .as_ref()
                .map_or_else(|| binding.root.clone(), |path| binding.root.join(path)),
        );
    }
    for binding in &bindings.owners {
        if !inventory
            .spec()
            .inputs
            .iter()
            .any(|input| input.owner_id == binding.owner_id)
            && !inventory
                .spec()
                .directories
                .iter()
                .any(|directory| directory.owner_id == binding.owner_id)
        {
            return Err(invalid("unreferenced local owner binding"));
        }
    }
    Ok(())
}
