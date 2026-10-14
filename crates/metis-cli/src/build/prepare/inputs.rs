//! Compiler-reported dep-info and build-script declarations determine real inputs.
use super::super::module::{CompilerOutput, ModuleBuild};
use super::{dep_info, metadata::Graph};
use crate::{Result, tree};
use metis_core::embedded_assets::{
    DirectoryPolicy, DirectoryRecord, DirectorySnapshot, FILE_LIMIT, FileIdentity, InputRecord,
    OwnerBinding, PAYLOAD_LIMIT,
};
mod artifact;
mod events;
#[cfg(test)]
mod tests;
struct InputSet {
    files: BTreeSet<PathBuf>,
    declared_directories: BTreeSet<(PathBuf, DirectoryPolicy)>,
    owners: Vec<OwnerBinding>,
    environment: BTreeSet<String>,
    features: BTreeSet<String>,
}
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

pub(super) struct Inputs {
    pub(super) records: Vec<InputRecord>,
    pub(super) owners: Vec<OwnerBinding>,
    pub(super) environment: Vec<(String, Option<String>)>,
    pub(super) features: Vec<String>,
    pub(super) directories: Vec<DirectoryRecord>,
}
pub(super) fn collect<State: CompilerOutput>(
    compilation: &ModuleBuild<State>,
    graph: &Graph,
    cargo_home: &Path,
) -> Result<Inputs> {
    collect_inner(
        compilation.state.messages(),
        compilation.state.root_dep_info(),
        &compilation.target,
        &compilation.context,
        graph,
        cargo_home,
    )
}
fn collect_inner(
    messages: &[u8],
    root_dep_info: Option<&Path>,
    target: &super::super::artifacts::ModuleTarget,
    context: &super::super::module::Context,
    graph: &Graph,
    cargo_home: &Path,
) -> Result<Inputs> {
    let mut input = InputSet {
        files: BTreeSet::new(),
        declared_directories: BTreeSet::new(),
        owners: graph.owners.clone(),
        environment: metis_core::embedded_assets::codegen_environment_keys(),
        features: BTreeSet::new(),
    };
    input.owners.push(OwnerBinding {
        owner_id: "cargo-home".into(),
        root: cargo_home.to_path_buf(),
    });
    input.files.extend([
        graph.workspace.join("Cargo.toml"),
        graph.workspace.join("Cargo.lock"),
    ]);
    input.files.extend(context.configurations.iter().cloned());
    package_manifests(graph, cargo_home, &mut input)?;
    events::collect(messages, target, root_dep_info, graph, &mut input)?;
    let directories =
        declared_directories(input.declared_directories, &input.owners, &mut input.files)?;
    let records = source_records(input.files, &input.owners)?;
    input.owners.retain(|owner| {
        records
            .iter()
            .any(|record| record.owner_id == owner.owner_id)
            || directories
                .iter()
                .any(|directory| directory.owner_id == owner.owner_id)
    });
    input
        .owners
        .sort_by(|left, right| left.owner_id.cmp(&right.owner_id));
    input.owners.dedup_by(|left, right| left == right);
    let environment = input
        .environment
        .into_iter()
        .map(|key| {
            let value = metis_core::embedded_assets::build_environment_value(
                &key,
                &graph.workspace,
                cargo_home,
            )?;
            Ok((key, value))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Inputs {
        records,
        owners: input.owners,
        environment,
        features: input.features.into_iter().collect(),
        directories,
    })
}
fn package_manifests(graph: &Graph, cargo_home: &Path, input: &mut InputSet) -> Result<()> {
    for (_, _, root) in &graph.packages {
        input.files.insert(root.join("Cargo.toml"));
        for directory in root.ancestors() {
            let manifest = directory.join("Cargo.toml");
            if manifest.is_file() {
                input.files.insert(manifest.canonicalize()?);
                if directory == graph.workspace {
                    break;
                }
                if directory.join(".git").exists() {
                    let package = graph
                        .packages
                        .iter()
                        .find(|(_, _, package_root)| package_root == root)
                        .ok_or("package owner disappeared")?;
                    input.owners.push(OwnerBinding {
                        owner_id: format!("repository:{}", package.1),
                        root: directory.to_path_buf(),
                    });
                    break;
                }
            }
            if directory == cargo_home {
                break;
            }
        }
    }
    Ok(())
}
fn declared_directories(
    declared: BTreeSet<(PathBuf, DirectoryPolicy)>,
    owners: &[OwnerBinding],
    files: &mut BTreeSet<PathBuf>,
) -> Result<Vec<DirectoryRecord>> {
    let mut directories = Vec::new();
    for (directory, policy) in declared {
        let binding = owner_for(&directory, owners)
            .ok_or("declared directory has no canonical source owner")?;
        let scope = (directory != binding.root)
            .then(|| tree::relative_name(&binding.root, &directory))
            .transpose()?;
        let snapshot = DirectorySnapshot::read(&binding.root, scope.as_deref(), policy)?;
        files.extend(
            snapshot
                .files()
                .iter()
                .map(|(path, _)| binding.root.join(path)),
        );
        directories.push(DirectoryRecord {
            owner_id: binding.owner_id.clone(),
            path: scope,
            policy,
            digest: snapshot.digest(),
        });
    }
    directories
        .sort_by(|left, right| (&left.owner_id, &left.path).cmp(&(&right.owner_id, &right.path)));
    Ok(directories)
}
fn source_records(files: BTreeSet<PathBuf>, owners: &[OwnerBinding]) -> Result<Vec<InputRecord>> {
    let mut records = BTreeMap::new();
    let mut total = 0_u64;
    for file in files {
        let file = metis_core::embedded_assets::canonical_file(&file)?;
        let binding = owner_for(&file, owners)
            .ok_or_else(|| format!("compiler input has no canonical owner: {}", file.display()))?;
        let path = tree::relative_name(&binding.root, &file)?;
        metis_core::embedded_assets::relative(&path)?;
        let identity = FileIdentity::read(&file, PAYLOAD_LIMIT)?;
        total = total
            .checked_add(identity.bytes)
            .ok_or("input payload length overflow")?;
        if total > PAYLOAD_LIMIT || records.len() >= FILE_LIMIT {
            return Err("compiler inputs exceed canonical record/payload bounds".into());
        }
        records.insert(
            (binding.owner_id.clone(), path.clone()),
            InputRecord {
                owner_id: binding.owner_id.clone(),
                path,
                identity,
            },
        );
    }
    let records = records.into_values().collect::<Vec<_>>();
    Ok(records)
}
fn owner_for<'a>(path: &Path, owners: &'a [OwnerBinding]) -> Option<&'a OwnerBinding> {
    owners
        .iter()
        .filter(|owner| path.starts_with(&owner.root))
        .max_by(|left, right| {
            left.root
                .components()
                .count()
                .cmp(&right.root.components().count())
                .then_with(|| right.owner_id.cmp(&left.owner_id))
        })
}
fn cargo_generated_environment(key: &str) -> bool {
    key.starts_with("CARGO_PKG_")
        || key.starts_with("CARGO_FEATURE_")
        || matches!(
            key,
            "CARGO_MANIFEST_DIR"
                | "CARGO_MANIFEST_PATH"
                | "CARGO_CRATE_NAME"
                | "OUT_DIR"
                | "OPT_LEVEL"
                | "PROFILE"
                | "HOST"
                | "TARGET"
                | "NUM_JOBS"
                | "DEBUG"
        )
}
