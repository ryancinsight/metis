//! Actual Cargo metadata determines locked providers and canonical owner roots.
use super::super::{artifacts, module::ModuleBuild};
use crate::Result;
use metis_core::embedded_assets::{Digest, OwnerBinding, ProviderIdentity, ProviderSource};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(super) struct Graph {
    pub(super) workspace: PathBuf,
    pub(super) providers: Vec<ProviderIdentity>,
    pub(super) owners: Vec<OwnerBinding>,
    pub(super) packages: Vec<(String, String, PathBuf)>,
}
pub(super) fn graph<State>(compilation: &ModuleBuild<State>) -> Result<Graph> {
    graph_metadata(&compilation.metadata, &compilation.target.package_id)
}
fn graph_metadata(metadata: &[u8], target_package: &str) -> Result<Graph> {
    let document: Value = serde_json::from_slice(metadata)?;
    let workspace = PathBuf::from(string(&document, "workspace_root")?).canonicalize()?;
    let package_values = document
        .get("packages")
        .and_then(Value::as_array)
        .ok_or("Cargo metadata omits packages")?;
    let members = document
        .get("workspace_members")
        .and_then(Value::as_array)
        .ok_or("Cargo metadata omits workspace members")?;
    let reached = artifacts::closure(&document, target_package)?;
    let lock_bytes = crate::bounded_read::read_file(
        &workspace.join("Cargo.lock"),
        metis_core::embedded_assets::MANIFEST_LIMIT,
        "Cargo.lock exceeds 1 MiB",
    )?;
    let lock: toml::Value = toml::from_str(std::str::from_utf8(&lock_bytes)?)?;
    let locked = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or("Cargo.lock omits packages")?;
    let mut providers = Vec::new();
    let mut owners = vec![OwnerBinding {
        owner_id: "workspace".into(),
        root: workspace.clone(),
    }];
    let mut packages = Vec::new();
    for id in reached {
        let package = package_values
            .iter()
            .find(|p| p.get("id") == Some(id))
            .ok_or("resolved package absent from Cargo metadata")?;
        let name = string(package, "name")?;
        let version = string(package, "version")?;
        let source = provider_source(package, locked, members.contains(id))?;
        let provider = ProviderIdentity::new(name.into(), version.into(), source);
        let package_id = provider.package_id.clone();
        let manifest = Path::new(string(package, "manifest_path")?).canonicalize()?;
        let root = manifest
            .parent()
            .ok_or("provider manifest has no parent")?
            .to_path_buf();
        packages.push((
            id.as_str().ok_or("Cargo package ID is not text")?.into(),
            package_id.clone(),
            root.clone(),
        ));
        owners.push(OwnerBinding {
            owner_id: package_id.clone(),
            root,
        });
        providers.push(provider);
    }
    providers.sort_by(|left, right| left.package_id.cmp(&right.package_id));
    owners.sort_by(|left, right| left.owner_id.cmp(&right.owner_id));
    Ok(Graph {
        workspace,
        providers,
        owners,
        packages,
    })
}
fn provider_source(
    package: &Value,
    locked: &[toml::Value],
    member: bool,
) -> Result<ProviderSource> {
    let name = string(package, "name")?;
    let version = string(package, "version")?;
    let source = match package.get("source").and_then(Value::as_str) {
        None if member => ProviderSource::Workspace,
        None => {
            return Err("module closure contains an unlocked nonworkspace path dependency".into());
        }
        Some(source) if source.starts_with("git+") => {
            let (url, revision) = source
                .rsplit_once('#')
                .ok_or("Git source omits locked revision")?;
            ProviderSource::Git {
                url: url.into(),
                revision: revision.into(),
            }
        }
        Some(source) if source.starts_with("registry+") => {
            let locked = locked
                .iter()
                .find(|p| {
                    p.get("name").and_then(toml::Value::as_str) == Some(name)
                        && p.get("version").and_then(toml::Value::as_str) == Some(version)
                        && p.get("source").and_then(toml::Value::as_str) == Some(source)
                })
                .ok_or("metadata registry provider is not current locked package")?;
            let checksum = locked
                .get("checksum")
                .and_then(toml::Value::as_str)
                .ok_or("registry lock omits checksum")?
                .parse::<Digest>()?;
            ProviderSource::Registry {
                url: source.into(),
                checksum,
            }
        }
        Some(_) => return Err("unsupported Cargo provider source kind".into()),
    };
    Ok(source)
}
pub(super) fn string<'value>(value: &'value Value, field: &str) -> Result<&'value str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Cargo metadata omitted text {field}").into())
}
