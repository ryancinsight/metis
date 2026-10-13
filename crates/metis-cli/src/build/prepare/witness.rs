//! A precompile source witness is projected onto the actual compiler input set.
//!
//! The producer observes S before real Cargo check, observes complete reported
//! inputs I (including check-generated files), then performs final code generation
//! and requires equal input identities and directory membership. Source S also
//! agrees with I's ordinary-file projection. These are bounded observations, not
//! an atomic filesystem snapshot: exclusive ownership during compilation is a
//! prerequisite, and a hostile transient A→B→A writer is outside this proof.
//! All relative source names use the canonical portable path admission. In
//! particular, literal Unix filename backslashes are excluded: in rustc's main
//! dependency-list grammar, a terminal backslash followed by its source separator
//! is indistinguishable from an escaped filename space. Native Windows separators
//! remain structural path components, and regular Windows file paths do not end
//! with one. No filesystem guessing recovers an unadmitted source name.
#[cfg(test)]
mod tests;
use super::super::module::Context;
use super::{inputs::Inputs, metadata::Graph};
use crate::Result;
use metis_core::embedded_assets::{
    DirectoryPolicy, DirectorySnapshot, FileIdentity, MANIFEST_LIMIT, PAYLOAD_LIMIT,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub(super) struct SourceWitness {
    files: BTreeMap<PathBuf, FileIdentity>,
}
impl SourceWitness {
    pub(super) fn capture(graph: &Graph, context: &Context) -> Result<Self> {
        let mut files = BTreeMap::new();
        let mut total = 0_u64;
        for (_, _, root) in &graph.packages {
            let snapshot = DirectorySnapshot::read(root, None, DirectoryPolicy::CargoPackage)?;
            for (path, identity) in snapshot.files() {
                insert(&mut files, root.join(path), *identity, &mut total)?;
            }
            for ancestor in root.ancestors() {
                let manifest = ancestor.join("Cargo.toml");
                if manifest.is_file() {
                    insert(
                        &mut files,
                        manifest.canonicalize()?,
                        FileIdentity::read(&manifest, MANIFEST_LIMIT)?,
                        &mut total,
                    )?;
                }
                if ancestor == graph.workspace || ancestor.join(".git").exists() {
                    // Workspace-level embedded files are direct members of the
                    // locked repository source domain, beyond crate roots.
                    for entry in fs::read_dir(ancestor)? {
                        let entry = entry?;
                        let path = entry.path();
                        if fs::symlink_metadata(&path)?.is_file() {
                            let identity = direct_source(ancestor, &path)?;
                            insert(&mut files, path, identity, &mut total)?;
                        }
                    }
                    break;
                }
            }
        }
        for path in &context.configurations {
            insert(
                &mut files,
                path.clone(),
                FileIdentity::read(path, MANIFEST_LIMIT)?,
                &mut total,
            )?;
        }
        Ok(Self { files })
    }
    pub(super) fn verify(&self, inputs: &Inputs, graph: &Graph) -> Result<()> {
        for input in &inputs.records {
            if input.owner_id.starts_with("generated:") || input.owner_id.starts_with("build:") {
                // Actual check generates these before final code generation;
                // prepare compares their complete identities across both passes.
                continue;
            }
            let owner = inputs
                .owners
                .iter()
                .find(|owner| owner.owner_id == input.owner_id)
                .ok_or("compiler input owner absent from source witness")?;
            let path = owner.root.join(&input.path);
            let before = self.files.get(&path).ok_or_else(|| {
                format!(
                    "compiler input was not captured before Cargo: {}",
                    path.display()
                )
            })?;
            if *before != input.identity {
                return Err(format!(
                    "source changed during Cargo compilation: {}",
                    path.display()
                )
                .into());
            }
        }
        let lock = graph.workspace.join("Cargo.lock");
        let before = self
            .files
            .get(&lock)
            .ok_or("Cargo.lock absent from precompile witness")?;
        if FileIdentity::read(&lock, MANIFEST_LIMIT)? != *before {
            return Err("Cargo.lock changed during compilation".into());
        }
        Ok(())
    }
}

fn direct_source(root: &Path, path: &Path) -> Result<FileIdentity> {
    let relative = crate::tree::relative_name(root, path)?;
    metis_core::embedded_assets::relative(&relative)?;
    Ok(FileIdentity::read(path, PAYLOAD_LIMIT)?)
}
fn insert(
    files: &mut BTreeMap<PathBuf, FileIdentity>,
    path: PathBuf,
    identity: FileIdentity,
    total: &mut u64,
) -> Result<()> {
    if let Some(previous) = files.get(&path) {
        if *previous != identity {
            return Err("source changed during precompile hashing".into());
        }
        return Ok(());
    }
    *total = total
        .checked_add(identity.bytes)
        .ok_or("precompile source payload overflow")?;
    if *total > PAYLOAD_LIMIT {
        return Err("precompile source witness exceeds 1 GiB".into());
    }
    files.insert(path, identity);
    Ok(())
}
